use crate::SCREEN_WIDTH;
use crate::ppu::regs::{DisplayControl, SpriteMapping};
use crate::ppu::sprite::{self, Attribute};
use crate::ppu::tile::{TextMapEntry, Tile};
use crate::ppu::{BackgroundView, BitmapLayout, Rgba, VideoMemoryView, palette_color, rgba};

pub(crate) type LayerLine = [Option<u16>; SCREEN_WIDTH as usize];
pub(crate) type SpriteLine = [Option<SpritePixel>; SCREEN_WIDTH as usize];
pub(crate) type FrameLine = [Rgba; SCREEN_WIDTH as usize];

#[derive(Clone, Copy)]
pub(crate) struct SpritePixel {
    pub(crate) color: u16,
    pub(crate) priority: u8,
}

pub(crate) fn composite(
    bg_order: &[(usize, u8)],
    bg_lines: &[LayerLine; 4],
    sprite_line: &SpriteLine,
    backdrop: u16,
    output: &mut FrameLine,
) {
    for (x, dst) in output.iter_mut().enumerate() {
        let (mut color, bg_priority) = bg_order
            .iter()
            .find_map(|&(index, priority)| bg_lines[index][x].map(|color| (color, priority)))
            .unwrap_or((backdrop, u8::MAX));

        if let Some(sprite) = sprite_line[x]
            && sprite.priority <= bg_priority
        {
            color = sprite.color;
        }

        *dst = rgba(color);
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
    let bg_y = (line + bg.vofs as usize) & (height - 1);

    for (x, dst) in output.iter_mut().enumerate() {
        let bg_x = (x + bg.hofs as usize) & (width - 1);

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

pub(crate) fn render_bitmap_line(
    layout: BitmapLayout,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut LayerLine,
) {
    if line >= layout.height {
        return;
    }

    let pitch = layout.width * layout.bytes_per_pixel;
    let start = layout.base + (line * pitch);
    let src = &memory.vram[start..start + pitch];
    for (dst, bytes) in output
        .iter_mut()
        .zip(src.chunks_exact(layout.bytes_per_pixel))
    {
        *dst = match *bytes {
            [0] => None,
            [index] => Some(palette_color(memory.palette, index.into())),
            _ => Some(u16::from_le_bytes([bytes[0], bytes[1]])),
        };
    }
}

pub(crate) fn render_sprite_line(
    display: DisplayControl,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut SpriteLine,
) {
    let mapping = display.sprite_mapping();
    for attr in sprite::attributes(memory.oam) {
        if attr.affine() {
            // TODO: affine shit
            continue;
        }

        if attr.disable_obj() {
            continue;
        }

        let (width, height) = attr.dimensions();
        let row = (line as u8).wrapping_sub(attr.y_coordinate()) as usize;
        if row >= height {
            continue;
        }

        let y = if attr.vertical_flip() {
            height - 1 - row
        } else {
            row
        };
        let x_start = attr.x_coordinate() as usize;
        for col in 0..width {
            let Some(dst) = output.get_mut((x_start + col) & 0x1FF) else {
                continue;
            };

            let x = if attr.horizontal_flip() {
                width - 1 - col
            } else {
                col
            };
            let Some(color) = sprite_texel(attr, mapping, memory, x, y) else {
                continue;
            };

            if dst.is_none_or(|pixel| attr.priority() < pixel.priority) {
                *dst = Some(SpritePixel {
                    color,
                    priority: attr.priority(),
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
