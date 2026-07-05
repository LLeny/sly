use crate::ppu::regs::{BackgroundMode, Priority};
use crate::ppu::{Ppu, SCREEN_WIDTH, Sprite, TilemapEntry, cgram};

#[derive(Clone, Copy)]
struct Pixel {
    addr: u8,
    layer: u8,
}

const OBJ: u8 = 4;
const BACKDROP: u8 = 5;

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
        let mut line: Line = [None; SCREEN_WIDTH as usize];
        let obj = if self.main_screen.obj() {
            self.obj_line(y)
        } else {
            [None; SCREEN_WIDTH as usize]
        };

        for &layer in self.layer_order() {
            match layer {
                Layer::Bg(bg, priority) => {
                    if self.main_screen.bg(bg) {
                        self.draw_bg(bg, priority, y, &mut line);
                    }
                }
                Layer::Obj(priority) => Self::draw_obj(priority, &obj, &mut line),
            }
        }

        self.resolve(scanline, &line);
    }

    // TODO
    fn resolve(&mut self, scanline: usize, line: &Line) {
        let row =
            &mut self.frame_buffer[scanline * SCREEN_WIDTH as usize..][..SCREEN_WIDTH as usize];

        for (dst, pixel) in row.iter_mut().zip(line) {
            let pixel = pixel.unwrap_or(Pixel {
                addr: 0,
                layer: BACKDROP,
            });

            let mut word = self.cgram.word(pixel.addr);
            if pixel.layer == BACKDROP && self.color_math.layers.backdrop() {
                word = self.color_math.apply(word);
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
                let pixels = self
                    .obj_tile(sprite.sub_tile(tx / 8, ty / 8))
                    .decode_row(ty as usize % 8, 4);

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

    fn draw_obj(priority: u8, obj: &ObjLine, line: &mut Line) {
        for (slot, &pixel) in line.iter_mut().zip(obj) {
            if slot.is_none()
                && let Some((p, addr)) = pixel
                && p == priority
            {
                *slot = Some(Pixel { addr, layer: OBJ });
            }
        }
    }

    fn draw_bg(&self, layer: usize, priority: bool, y: u32, line: &mut Line) {
        let (Some(bpp), Some(map)) = (self.bg_mode.bpp(layer), self.tilemap_header(layer)) else {
            return;
        };

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
                    pixels = self
                        .bg_tile(layer, index)
                        .unwrap_or_default()
                        .decode_row(py as usize % 8, bpp);
                    cached = Some((index, py));
                }

                let color = pixels[px as usize % 8];
                if color != 0 {
                    *slot = Some(Pixel {
                        addr: (base + color as usize) as u8,
                        layer: layer as u8,
                    });
                }
            }

            x += run;
        }
    }
}
