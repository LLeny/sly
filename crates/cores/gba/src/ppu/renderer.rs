use crate::SCREEN_WIDTH;
use crate::ppu::affine::AffineMatrix;
use crate::ppu::blend::{BlendView, LayerPixel};
use crate::ppu::mosaic::MosaicSize;
use crate::ppu::regs::{BACKDROP_LAYER, DisplayControl, OBJ_LAYER, SpriteMapping};
use crate::ppu::sprite::{self, Attribute, ObjMode};
use crate::ppu::tile::{TextMapEntry, Tile};
use crate::ppu::window::WindowView;
use crate::ppu::{BackgroundView, BitmapLayout, Rgba, VideoMemoryView, palette_color, rgba};

pub(crate) type LayerLine = [Option<u16>; SCREEN_WIDTH as usize];
pub(crate) type SpriteLine = [Option<SpritePixel>; SCREEN_WIDTH as usize];
pub(crate) type ObjWindowLine = [bool; SCREEN_WIDTH as usize];
pub(crate) type FrameLine = [Rgba; SCREEN_WIDTH as usize];

#[derive(Clone, Copy)]
pub(crate) struct SpritePixel {
    pub(crate) color: u16,
    pub(crate) priority: u8,
    pub(crate) semi_transparent: bool,
}

pub(crate) fn composite(
    bg_order: &[(usize, u8)],
    bg_lines: &[LayerLine; 4],
    sprite_line: &SpriteLine,
    obj_window: &ObjWindowLine,
    backdrop: u16,
    windows: WindowView,
    blending: BlendView,
    line: usize,
    output: &mut FrameLine,
) {
    let backdrop = LayerPixel {
        color: backdrop,
        layer: BACKDROP_LAYER,
        priority: u8::MAX,
        semi_transparent: false,
    };

    for (x, dst) in output.iter_mut().enumerate() {
        let mask = windows.mask(x, line, obj_window[x]);
        let mut layers = bg_order
            .iter()
            .filter(|&&(index, _)| mask.bg_enabled(index))
            .filter_map(|&(index, priority)| {
                bg_lines[index][x].map(|color| LayerPixel {
                    color,
                    layer: index,
                    priority,
                    semi_transparent: false,
                })
            })
            .chain(std::iter::once(backdrop));

        let mut top = layers.next().unwrap();
        let mut below = layers.next();

        if let Some(sprite) = sprite_line[x]
            && mask.obj_enabled()
        {
            let obj = LayerPixel {
                color: sprite.color,
                layer: OBJ_LAYER,
                priority: sprite.priority,
                semi_transparent: sprite.semi_transparent,
            };
            if obj.priority <= top.priority {
                below = Some(std::mem::replace(&mut top, obj));
            } else if below.is_none_or(|pixel| obj.priority <= pixel.priority) {
                below = Some(obj);
            }
        }

        *dst = rgba(blending.apply(top, below, mask.effects_enabled()));
    }
}

pub(crate) fn render_background_line(
    bg: BackgroundView,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut LayerLine,
) {
    let Some((width, height)) = bg.ctrl.screen_size().text_dimensions() else {
        return;
    };

    let depth = bg.ctrl.color_depth();
    let bg_y = (bg.mosaic.y(line) + bg.vofs as usize) & (height - 1);

    for (x, dst) in output.iter_mut().enumerate() {
        let bg_x = (bg.mosaic.x(x) + bg.hofs as usize) & (width - 1);

        let entry_addr = bg.ctrl.text_entry_offset(bg_x / 8, bg_y / 8, width);
        let entry = TextMapEntry::from_le_bytes(
            memory.vram[entry_addr..entry_addr + 2].try_into().unwrap(),
        );

        let tile_addr = bg.ctrl.tile_offset(entry.tile_number() as usize, depth);
        let tile = Tile {
            data: &memory.vram[tile_addr..tile_addr + depth.tile_size()],
            depth,
        };

        let index = entry.pixel(&tile, bg_x % 8, bg_y % 8) as usize;
        if index != 0 {
            *dst = Some(palette_color(
                memory.palette,
                entry.palette_base(depth) + index,
            ));
        }
    }
}

pub(crate) fn render_affine_background_line(
    bg: BackgroundView,
    matrix: AffineMatrix,
    (mut x, mut y): (i32, i32),
    memory: VideoMemoryView<'_>,
    output: &mut LayerLine,
) {
    let Some((size, _)) = bg.ctrl.dimensions(bg.kind) else {
        return;
    };
    let depth = bg.ctrl.effective_color_depth(bg.kind);
    // affine maps are always a power of two in size, so wrapping is a mask
    let wrap_mask = if bg.ctrl.wraparound() {
        size - 1
    } else {
        usize::MAX
    };

    for dst in output.iter_mut() {
        let tx = ((x >> 8) as usize) & wrap_mask;
        let ty = ((y >> 8) as usize) & wrap_mask;
        x += matrix.pa;
        y += matrix.pc;

        if tx >= size || ty >= size {
            continue;
        }

        let Some((_, entry)) = bg
            .ctrl
            .map_entry(bg.kind, memory.vram, tx / 8, ty / 8, size)
        else {
            continue;
        };
        let tile_addr = bg.ctrl.tile_offset(entry.tile_number() as usize, depth);
        let Some(data) = memory.vram.get(tile_addr..(tile_addr + depth.tile_size())) else {
            continue;
        };

        let tile = Tile { data, depth };
        let index = tile.pixel(tx % 8, ty % 8) as usize;
        if index != 0 {
            *dst = Some(palette_color(memory.palette, index));
        }
    }
}

pub(crate) fn render_bitmap_line(
    layout: BitmapLayout,
    mosaic: MosaicSize,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut LayerLine,
) {
    if line >= layout.height {
        return;
    }

    let pitch = layout.width * layout.bytes_per_pixel;
    let start = layout.base + (mosaic.y(line) * pitch);
    let src = &memory.vram[start..start + pitch];
    for (x, dst) in output.iter_mut().enumerate().take(layout.width) {
        let offset = mosaic.x(x) * layout.bytes_per_pixel;
        let bytes = &src[offset..offset + layout.bytes_per_pixel];
        *dst = match *bytes {
            [0] => None,
            [index] => Some(palette_color(memory.palette, index.into())),
            _ => Some(u16::from_le_bytes([bytes[0], bytes[1]])),
        };
    }
}

pub(crate) fn render_sprite_line(
    display: DisplayControl,
    mosaic: MosaicSize,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut SpriteLine,
    obj_window: &mut ObjWindowLine,
) {
    if !display.enable_obj() {
        return;
    }

    let mapping = display.sprite_mapping();
    for attr in sprite::attributes(memory.oam) {
        if !attr.affine() && attr.disable_obj() {
            continue;
        }

        let is_window = match attr.mode() {
            ObjMode::ObjWindow if display.enable_obj_window() => true,
            ObjMode::Normal | ObjMode::SemiTransparent => false,
            _ => continue,
        };

        let (width, height) = attr.dimensions();
        let (box_width, box_height) = attr.bounds();

        let row = (line as u8).wrapping_sub(attr.y_coordinate()) as usize;
        if row >= box_height {
            continue;
        }

        let mosaic = if attr.mosaic() {
            mosaic
        } else {
            MosaicSize::NONE
        };
        let row = row - (line - mosaic.y(line)).min(row);

        let matrix = attr
            .affine()
            .then(|| AffineMatrix::from_oam(memory.oam, attr.matrix_index() as usize));
        let dy = row as i32 - (box_height / 2) as i32;
        let x_start = attr.x_coordinate() as usize;
        for col in 0..box_width {
            let screen_x = (x_start + col) & 0x1FF;
            let Some(dst) = output.get_mut(screen_x) else {
                continue;
            };

            let col = col - (screen_x - mosaic.x(screen_x)).min(col);
            let (x, y) = if let Some(matrix) = matrix {
                let dx = col as i32 - (box_width / 2) as i32;
                let (tx, ty) = matrix.apply(dx, dy);
                let x = tx + (width / 2) as i32;
                let y = ty + (height / 2) as i32;

                let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
                    continue;
                };

                if x >= width || y >= height {
                    continue;
                }

                (x, y)
            } else {
                attr.flip(col, row)
            };

            let Some(color) = sprite_texel(attr, mapping, memory, x, y) else {
                continue;
            };

            if is_window {
                obj_window[screen_x] = true;
                continue;
            }

            if dst.is_none_or(|pixel| attr.priority() < pixel.priority) {
                *dst = Some(SpritePixel {
                    color,
                    priority: attr.priority(),
                    semi_transparent: matches!(attr.mode(), ObjMode::SemiTransparent),
                })
            }
        }
    }
}

pub(crate) fn sprite_texel(
    attr: Attribute,
    mapping: SpriteMapping,
    memory: VideoMemoryView<'_>,
    x: usize,
    y: usize,
) -> Option<u16> {
    let depth = attr.color_depth();
    let addr = attr.tile_addr(mapping, x / 8, y / 8);
    let data = memory.vram.get(addr..(addr + depth.tile_size()))?;
    let tile = Tile { data, depth };
    let index = tile.pixel(x % 8, y % 8) as usize;
    if index == 0 {
        return None; // transparent
    }

    Some(palette_color(memory.palette, attr.palette_base() + index))
}
