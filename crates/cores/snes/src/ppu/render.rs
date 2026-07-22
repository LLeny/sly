use crate::ppu::layer_order::{self, RenderPass};
use crate::ppu::regs::{
    ColorMathAddend, ColorMathIntensity, ColorSource, LayerMask, Priority, WindowRegion,
};
use crate::ppu::{Ppu, SCREEN_WIDTH, Sprite, TilemapEntry, cgram};

const SCREEN_WIDTH_USIZE: usize = SCREEN_WIDTH as usize;
const MAX_OBJECTS_PER_SCANLINE: usize = 32;
const MAX_OBJECT_TILES_PER_SCANLINE: usize = 34;
const FIRST_COLOR_MATH_OBJECT_PALETTE: u8 = 4;

type CompositedLine = [Option<VisiblePixel>; SCREEN_WIDTH_USIZE];
type ObjectLine = [Option<ObjectPixel>; SCREEN_WIDTH_USIZE];
type RgbLine = [(u8, u8, u8); SCREEN_WIDTH_USIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VisiblePixel {
    color: u16,
    source: PixelSource,
    allows_color_math: bool,
}

impl VisiblePixel {
    fn backdrop(color: u16) -> Self {
        Self {
            color,
            source: PixelSource::Backdrop,
            allows_color_math: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PixelSource {
    Background(u8),
    Object,
    Backdrop,
}

impl PixelSource {
    fn color_math_layer(self) -> u8 {
        match self {
            Self::Background(layer) => layer,
            Self::Object => LayerMask::OBJ,
            Self::Backdrop => LayerMask::BACKDROP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObjectPixel {
    priority: u8,
    palette: u8,
    color: u8,
}

struct ScanlineRenderer<'a> {
    ppu: &'a Ppu,
    y: u32,
}

impl<'a> ScanlineRenderer<'a> {
    fn new(ppu: &'a Ppu, y: u32) -> Self {
        Self { ppu, y }
    }

    fn render(&self, row: &mut RgbLine) {
        let objects = if self.ppu.main_screen.obj() || self.ppu.sub_screen.obj() {
            self.object_pixels_on_scanline()
        } else {
            [None; SCREEN_WIDTH_USIZE]
        };

        let main = self.compose_layers(self.ppu.main_screen, &objects);
        let sub = self
            .uses_sub_screen_for_color_math()
            .then(|| self.compose_layers(self.ppu.sub_screen, &objects));

        self.resolve_output_scanline(&main, sub.as_ref(), row);
    }

    fn uses_sub_screen_for_color_math(&self) -> bool {
        self.ppu.color_math.addend == ColorMathAddend::SubScreen
            && self.ppu.color_math.layers.raw() != 0
    }

    fn compose_layers(&self, enabled_layers: LayerMask, objects: &ObjectLine) -> CompositedLine {
        let mut line = [None; SCREEN_WIDTH_USIZE];

        let passes = layer_order::front_to_back_passes(self.ppu.bg_mode, self.ppu.bg3_priority);
        for pass in passes {
            match *pass {
                RenderPass::Background { layer, priority } => {
                    if enabled_layers.bg(layer) {
                        self.paint_background(layer, priority, &mut line);
                    }
                }
                RenderPass::Objects { priority } => {
                    if enabled_layers.obj() {
                        self.paint_objects(priority, objects, &mut line);
                    }
                }
            }
        }

        line
    }

    fn visible_objects_on_scanline(&self) -> [Option<Sprite>; MAX_OBJECTS_PER_SCANLINE] {
        // TODO: apply OAM priority rotation
        let mut objects = [None; MAX_OBJECTS_PER_SCANLINE];
        let mut count = 0;

        for index in 0..128 {
            if !self.ppu.sprite_covers_line(index, self.y) {
                continue;
            }

            let sprite = self.ppu.sprite(index);
            if !sprite.in_range(self.y) {
                continue;
            }

            if count == objects.len() {
                // TODO: set the range-over flag
                break;
            }

            objects[count] = Some(sprite);
            count += 1;
        }

        objects
    }

    fn object_pixels_on_scanline(&self) -> ObjectLine {
        let mut line = [None; SCREEN_WIDTH_USIZE];
        let mut tile_count = 0;

        for sprite in self.visible_objects_on_scanline().iter().rev().flatten() {
            let sprite_row = sprite.row(self.y);

            for tile_column in 0..sprite.width / 8 {
                let screen_x = sprite.x + tile_column as i32 * 8;
                if screen_x <= -8 || screen_x >= SCREEN_WIDTH as i32 {
                    continue;
                }

                if tile_count == MAX_OBJECT_TILES_PER_SCANLINE {
                    // TODO: set the time-over flag?
                    return line;
                }
                tile_count += 1;

                let (texture_x, texture_y) = sprite.texel(tile_column * 8, sprite_row);
                let tile = sprite.sub_tile(texture_x / 8, texture_y / 8);
                let tile_base = self.ppu.obj.tile_addr(tile) * 2;
                let tile_row = self.ppu.tile_row(tile_base, texture_y as usize % 8, 4);

                for pixel_offset in 0..8u32 {
                    let x = screen_x + pixel_offset as i32;
                    if !(0..SCREEN_WIDTH as i32).contains(&x) {
                        continue;
                    }

                    let (texture_x, _) = sprite.texel(tile_column * 8 + pixel_offset, sprite_row);
                    let color_index = tile_row[texture_x as usize % 8];
                    if color_index != 0 {
                        line[x as usize] = Some(ObjectPixel {
                            priority: sprite.priority,
                            palette: sprite.palette,
                            color: color_index,
                        });
                    }
                }
            }
        }

        line
    }

    fn paint_objects(&self, priority: u8, objects: &ObjectLine, line: &mut CompositedLine) {
        for (slot, object) in line.iter_mut().zip(objects) {
            let Some(object) = object else {
                continue;
            };
            if slot.is_some() || object.priority != priority {
                continue;
            }

            *slot = Some(self.visible_object_pixel(*object));
        }
    }

    fn visible_object_pixel(&self, object: ObjectPixel) -> VisiblePixel {
        VisiblePixel {
            color: self
                .ppu
                .cgram
                .word(Ppu::obj_palette_addr(object.palette, object.color)),
            source: PixelSource::Object,
            allows_color_math: object.palette >= FIRST_COLOR_MATH_OBJECT_PALETTE,
        }
    }

    fn paint_background(&self, layer: usize, priority: Priority, line: &mut CompositedLine) {
        let (Some(bits_per_pixel), Some(map)) =
            (self.ppu.bg_mode.bpp(layer), self.ppu.tilemap_header(layer))
        else {
            return;
        };

        let use_direct_color =
            bits_per_pixel == 8 && matches!(self.ppu.color_math.color_source, ColorSource::Direct);
        let sampled_y = self.ppu.mosaic.sample(layer, self.y);
        let map_y = (sampled_y + map.scroll_y) % (map.height * map.tile_px);
        let (tile_y, pixel_y) = (map_y / map.tile_px, map_y % map.tile_px);

        // Adjacent pixels usually come from the same tile and decoded row. With
        // mosaic enabled, all pixels in a horizontal block reuse the same
        // sampled tile and texel as well.
        let mut cached_entry = None;
        let mut cached_tile_row = None;
        let mut decoded_row = [0u8; 8];
        let mut entry = TilemapEntry::from(0);

        for screen_x in 0..SCREEN_WIDTH {
            let sampled_x = self.ppu.mosaic.sample(layer, screen_x);
            let map_x = (sampled_x + map.scroll_x) % (map.width * map.tile_px);
            let (tile_x, pixel_x) = (map_x / map.tile_px, map_x % map.tile_px);

            if cached_entry != Some((tile_x, tile_y)) {
                entry =
                    TilemapEntry::from(self.ppu.vram.word(map.entry_addr(tile_x, tile_y) as u16));
                cached_entry = Some((tile_x, tile_y));
            }
            if entry.priority != priority {
                continue;
            }

            let slot = &mut line[screen_x as usize];
            if slot.is_some() {
                continue;
            }

            let (texture_x, texture_y) = entry.texel(pixel_x, pixel_y, map.tile_px);
            let tile_index = entry.char_index(texture_x, texture_y);
            if cached_tile_row != Some((tile_index, texture_y)) {
                let tile_base = self.ppu.bg_layers[layer].tileset_addr * 2
                    + tile_index as u32 * (bits_per_pixel as u32 * 8);
                decoded_row = self
                    .ppu
                    .tile_row(tile_base, texture_y as usize % 8, bits_per_pixel);
                cached_tile_row = Some((tile_index, texture_y));
            }

            let color_index = decoded_row[texture_x as usize % 8];
            if color_index == 0 {
                continue;
            }

            let (palette_base, _) =
                self.ppu
                    .bg_palette(layer, bits_per_pixel, entry.palette_selection as usize);
            *slot = Some(VisiblePixel {
                color: self.background_color(palette_base, &entry, color_index, use_direct_color),
                source: PixelSource::Background(layer as u8),
                allows_color_math: true,
            });
        }
    }

    fn background_color(
        &self,
        palette_base: usize,
        entry: &TilemapEntry,
        color_index: u8,
        use_direct_color: bool,
    ) -> u16 {
        if use_direct_color {
            cgram::direct_color(entry.palette_selection, color_index)
        } else {
            self.ppu
                .cgram
                .word((palette_base + color_index as usize) as u8)
        }
    }

    fn resolve_output_scanline(
        &self,
        main: &CompositedLine,
        sub: Option<&CompositedLine>,
        row: &mut RgbLine,
    ) {
        let backdrop = VisiblePixel::backdrop(self.ppu.cgram.word(0));

        // TODO: Windows, HDMA?
        let force_main_to_black = matches!(
            self.ppu.color_math.main_screen_region,
            WindowRegion::Everywhere
        );
        let color_math_masked = matches!(
            self.ppu.color_math.sub_screen_region,
            WindowRegion::Everywhere
        );

        for (x, out) in row.iter_mut().enumerate() {
            *out = self.resolve_output_pixel(
                main[x].unwrap_or(backdrop),
                sub.and_then(|line| line[x]),
                force_main_to_black,
                color_math_masked,
            );
        }
    }

    fn resolve_output_pixel(
        &self,
        pixel: VisiblePixel,
        sub: Option<VisiblePixel>,
        force_main_to_black: bool,
        color_math_masked: bool,
    ) -> (u8, u8, u8) {
        let mut color = if force_main_to_black { 0 } else { pixel.color };

        if pixel.allows_color_math
            && self
                .ppu
                .color_math
                .layers
                .layer(pixel.source.color_math_layer())
            && !color_math_masked
        {
            let (addend, sub_screen_was_transparent) = self.color_math_addend(sub);
            let halve = matches!(self.ppu.color_math.intensity, ColorMathIntensity::Half)
                && !force_main_to_black
                && !sub_screen_was_transparent;
            color = self.ppu.color_math.apply(color, addend, halve);
        }

        cgram::rgb(color)
    }

    /// Returns addend and whether the sub was transparent.
    fn color_math_addend(&self, sub: Option<VisiblePixel>) -> (u16, bool) {
        match (self.ppu.color_math.addend, sub) {
            (ColorMathAddend::SubScreen, Some(pixel)) => (pixel.color, false),
            (ColorMathAddend::SubScreen, None) => (self.ppu.color_math.fixed_color, true),
            _ => (self.ppu.color_math.fixed_color, false),
        }
    }
}

impl Ppu {
    pub fn render_scanline(&mut self, scanline: usize) {
        let mut row = [(0, 0, 0); SCREEN_WIDTH_USIZE];

        if !self.force_blank {
            ScanlineRenderer::new(self, scanline as u32).render(&mut row);

            if self.brightness != 15 {
                for pixel in &mut row {
                    *pixel = Self::apply_brightness(*pixel, self.brightness);
                }
            }
        }

        self.frame_buffer[scanline * SCREEN_WIDTH_USIZE..][..SCREEN_WIDTH_USIZE]
            .copy_from_slice(&row);
    }

    fn apply_brightness((r, g, b): (u8, u8, u8), brightness: u8) -> (u8, u8, u8) {
        let scale = |channel: u8| (channel as u16 * brightness as u16 / 15) as u8;
        (scale(r), scale(g), scale(b))
    }
}
