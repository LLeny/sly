use crate::ppu::regs::{
    BackgroundMode, ColorMathAddend, ColorMathIntensity, ColorSource, LayerMask, Priority,
};
use crate::ppu::{Ppu, SCREEN_WIDTH, Sprite, TilemapEntry, cgram};

#[derive(Clone, Copy)]
struct Pixel {
    word: u16,
    layer: u8,
    math: bool,
}

const OBJ: u8 = LayerMask::OBJ;
const BACKDROP: u8 = LayerMask::BACKDROP;

const FIRST_MATH_OBJ_PALETTE: u8 = 4;

type Line = [Option<Pixel>; SCREEN_WIDTH as usize];
type ObjLine = [Option<(u8, u8)>; SCREEN_WIDTH as usize];

#[derive(Clone, Copy)]
enum Layer {
    Bg(usize, bool),
    Obj(u8),
}

impl Ppu {
    pub fn render_scanline(&mut self, scanline: usize) {
        let y = scanline as u32;
        let obj = if self.main_screen.obj() || self.sub_screen.obj() {
            self.obj_line(y)
        } else {
            [None; SCREEN_WIDTH as usize]
        };

        let main = self.composite(self.main_screen, &obj, y);
        let sub = if self.color_math.addend == ColorMathAddend::SubScreen
            && self.color_math.layers.raw() != 0
        {
            Some(self.composite(self.sub_screen, &obj, y))
        } else {
            None
        };

        self.resolve(scanline, &main, sub.as_ref());
    }

    fn composite(&self, screen: LayerMask, obj: &ObjLine, y: u32) -> Line {
        let mut line: Line = [None; SCREEN_WIDTH as usize];

        for &layer in self.layer_order() {
            match layer {
                Layer::Bg(bg, priority) => {
                    if screen.bg(bg) {
                        self.draw_bg(bg, priority, y, &mut line);
                    }
                }
                Layer::Obj(priority) => {
                    if screen.obj() {
                        self.draw_obj(priority, obj, &mut line);
                    }
                }
            }
        }

        line
    }

    fn resolve(&mut self, scanline: usize, main: &Line, sub: Option<&Line>) {
        let math = &self.color_math;

        // TODO: windows
        let in_window = false;
        let force_black = math.main_screen_region.contains(in_window);
        let math_masked = math.sub_screen_region.contains(in_window);

        let backdrop = Pixel {
            word: self.cgram.word(0),
            layer: BACKDROP,
            math: true,
        };

        let row =
            &mut self.frame_buffer[scanline * SCREEN_WIDTH as usize..][..SCREEN_WIDTH as usize];

        for (x, (dst, pixel)) in row.iter_mut().zip(main).enumerate() {
            let pixel = pixel.unwrap_or(backdrop);
            let mut word = if force_black { 0 } else { pixel.word };

            if pixel.math && math.layers.layer(pixel.layer) && !math_masked {
                let (addend, transparent_sub) = match (math.addend, sub) {
                    (ColorMathAddend::SubScreen, Some(line)) => match line[x] {
                        Some(sub_pixel) => (sub_pixel.word, false),
                        None => (math.fixed_color, true),
                    },
                    _ => (math.fixed_color, false),
                };
                let halve = matches!(math.intensity, ColorMathIntensity::Half)
                    && !force_black
                    && !transparent_sub;
                word = math.apply(word, addend, halve);
            }

            *dst = cgram::rgb(word);
        }
    }

    fn layer_order(&self) -> &'static [Layer] {
        use Layer::{Bg, Obj};

        const H: bool = true;
        const L: bool = false;

        const TWO_BG: &[Layer] = &[
            Obj(3),
            Bg(0, H),
            Obj(2),
            Bg(1, H),
            Obj(1),
            Bg(0, L),
            Obj(0),
            Bg(1, L),
        ];

        match (self.bg_mode, self.bg3_priority) {
            (BackgroundMode::Mode0, _) => &[
                Obj(3),
                Bg(0, H),
                Bg(1, H),
                Obj(2),
                Bg(0, L),
                Bg(1, L),
                Obj(1),
                Bg(2, H),
                Bg(3, H),
                Obj(0),
                Bg(2, L),
                Bg(3, L),
            ],
            (BackgroundMode::Mode1, Priority::High) => &[
                Bg(2, H),
                Obj(3),
                Bg(0, H),
                Bg(1, H),
                Obj(2),
                Bg(0, L),
                Bg(1, L),
                Obj(1),
                Obj(0),
                Bg(2, L),
            ],
            (BackgroundMode::Mode1, Priority::Normal) => &[
                Obj(3),
                Bg(0, H),
                Bg(1, H),
                Obj(2),
                Bg(0, L),
                Bg(1, L),
                Obj(1),
                Bg(2, H),
                Obj(0),
                Bg(2, L),
            ],
            // TODO: modes 2-7
            _ => TWO_BG,
        }
    }

    fn sprites_in_range(&self, y: u32) -> [Option<Sprite>; 32] {
        // TODO: priority rotation
        let mut in_range = [None; 32];
        let mut count = 0;

        for index in 0..128 {
            if !self.sprite_covers_line(index, y) {
                continue;
            }

            let sprite = self.sprite(index);
            if !sprite.in_range(y) {
                continue;
            }

            if count == in_range.len() {
                // TODO: range over flag
                break;
            }

            in_range[count] = Some(sprite);
            count += 1;
        }

        in_range
    }

    fn obj_line(&self, y: u32) -> ObjLine {
        let mut line = [None; SCREEN_WIDTH as usize];
        let mut tiles = 0;

        for sprite in self.sprites_in_range(y).iter().rev().flatten() {
            let row = sprite.row(y);

            for sliver in 0..sprite.width / 8 {
                let base = sprite.x + sliver as i32 * 8;
                if base <= -8 || base >= SCREEN_WIDTH as i32 {
                    continue;
                }

                if tiles == 34 {
                    // TODO: time over flag
                    return line;
                }

                tiles += 1;

                let (tx, ty) = sprite.texel(sliver * 8, row);
                let tile_base = self.obj.tile_addr(sprite.sub_tile(tx / 8, ty / 8)) * 2;
                let pixels = self.tile_row(tile_base, ty as usize % 8, 4);

                for i in 0..8u32 {
                    let x = base + i as i32;
                    if !(0..SCREEN_WIDTH as i32).contains(&x) {
                        continue;
                    }

                    let (px, _) = sprite.texel(sliver * 8 + i, row);
                    let color = pixels[px as usize % 8];
                    if color != 0 {
                        line[x as usize] = Some((
                            sprite.priority,
                            Self::obj_palette_addr(sprite.palette, color),
                        ));
                    }
                }
            }
        }

        line
    }

    fn draw_obj(&self, priority: u8, obj: &ObjLine, line: &mut Line) {
        for (slot, &pixel) in line.iter_mut().zip(obj) {
            if slot.is_none()
                && let Some((p, addr)) = pixel
                && p == priority
            {
                *slot = Some(Pixel {
                    word: self.cgram.word(addr),
                    layer: OBJ,
                    math: addr >= Self::obj_palette_addr(FIRST_MATH_OBJ_PALETTE, 0),
                });
            }
        }
    }

    fn draw_bg(&self, layer: usize, priority: bool, y: u32, line: &mut Line) {
        let (Some(bpp), Some(map)) = (self.bg_mode.bpp(layer), self.tilemap_header(layer)) else {
            return;
        };

        let direct = bpp == 8 && matches!(self.color_math.color_source, ColorSource::Direct);
        let sy = (y + map.scroll_y) % (map.height * map.tile_px);
        let (ty, in_y) = (sy / map.tile_px, sy % map.tile_px);

        let mut cached = None;
        let mut pixels = [0u8; 8];

        let mut x = 0;
        while x < line.len() as u32 {
            let sx = (x + map.scroll_x) % (map.width * map.tile_px);
            let (tx, in_x) = (sx / map.tile_px, sx % map.tile_px);
            let run = (map.tile_px - in_x).min(line.len() as u32 - x);

            let entry = TilemapEntry::from(self.vram.word(map.entry_addr(tx, ty) as u16));
            if entry.priority != priority {
                x += run;
                continue;
            }

            let (base, _) = self.bg_palette(layer, bpp, entry.palette_selection as usize);

            for i in 0..run {
                let slot = &mut line[(x + i) as usize];
                if slot.is_some() {
                    continue;
                }

                let (px, py) = entry.texel(in_x + i, in_y, map.tile_px);
                let index = entry.char_index(px, py);
                if cached != Some((index, py)) {
                    let tile_base =
                        self.bg_layers[layer].tileset_addr * 2 + index as u32 * (bpp as u32 * 8);
                    pixels = self.tile_row(tile_base, py as usize % 8, bpp);
                    cached = Some((index, py));
                }

                let color = pixels[px as usize % 8];
                if color != 0 {
                    let word = if direct {
                        cgram::direct_color(entry.palette_selection, color)
                    } else {
                        self.cgram.word((base + color as usize) as u8)
                    };

                    *slot = Some(Pixel {
                        word,
                        layer: layer as u8,
                        math: true,
                    });
                }
            }

            x += run;
        }
    }
}
