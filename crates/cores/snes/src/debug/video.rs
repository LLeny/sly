use super::*;
use std::hash::{DefaultHasher, Hasher};

pub(super) fn ppu_view(core: &Snes) -> StructuredView {
    use crate::ppu::regs::{Priority, TileSize};

    let trace = core.ppu.window_trace.rows();
    let mut mode_names = Vec::new();
    let mut ranges = Vec::new();
    let mut range_start = 0;
    for line in 1..=trace.len() {
        if line == trace.len()
            || trace[line].bg_mode as u8 != trace[range_start].bg_mode as u8
            || trace[line].extbg != trace[range_start].extbg
        {
            let state = trace[range_start];
            let name = background_mode_name(state.bg_mode, state.extbg);
            if !mode_names.contains(&name) {
                mode_names.push(name.clone());
            }

            ranges.push(format!("{name} y{}-{}", range_start + 1, line));
            range_start = line;
        }
    }

    let mode = if mode_names.is_empty() {
        background_mode_name(core.ppu.bg_mode, core.ppu.extbg)
    } else {
        mode_names.join(" + ")
    };
    let bg3_priority = match core.ppu.bg3_priority {
        Priority::Normal => "normal",
        Priority::High => "high",
    };

    let mut properties = vec![
        Field::new("BG mode", mode).accented(),
        Field::new("BG3 priority", bg3_priority),
    ];

    if ranges.len() > 1 {
        const SHOWN: usize = 6;
        let extra = ranges.len().saturating_sub(SHOWN);
        for (i, range) in ranges.iter().take(SHOWN).enumerate() {
            let label = if i == 0 { "Bitmap" } else { "" };
            properties.push(Field::new(label, range.clone()));
        }

        if extra > 0 {
            properties.push(Field::new("", format!("+ {extra} more")));
        }
    }

    let groups: Vec<_> = core
        .ppu
        .bg_layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let mode7_layer = mode7_bg_layer(core, i);
            let tile_size = if mode7_layer {
                "8x8"
            } else {
                match layer.tile_size {
                    TileSize::Size8x8 => "8x8",
                    TileSize::Size16x16 => "16x16",
                }
            };
            let depth = raster_depth(core, i);
            let tileset: Arc<str> =
                hex(if mode7_layer { 0 } else { layer.tileset_addr }, 16).into();
            let tilemap: Arc<str> =
                hex(if mode7_layer { 0 } else { layer.tilemap_addr }, 16).into();
            let name = bg_layer_name(core, i);

            Row::Group {
                id: format!("ppu-bg{}", i + 1).into(),
                title: name.into(),
                summary: tileset.clone(),
                active: false,
                rows: vec![
                    Row::Field(Field::new("Tileset", tileset)),
                    Row::Field(Field::new("Tilemap", tilemap)),
                    Row::Field(Field::new("Tile size", tile_size)),
                    Row::Field(Field::new("Depth", depth)),
                ],
            }
        })
        .collect();

    let mut rows: Vec<_> = properties.into_iter().map(Row::Field).collect();
    rows.extend(groups);
    StructuredView {
        sections: vec![Section {
            rows,
            ..Default::default()
        }],
        meta: None,
    }
}

pub(super) fn background_mode_name(mode: crate::ppu::regs::BackgroundMode, extbg: bool) -> String {
    use crate::ppu::regs::BackgroundMode;

    match mode {
        BackgroundMode::Invalid => "Invalid".to_string(),
        BackgroundMode::Mode7 if extbg => "Mode 7 EXTBG".to_string(),
        mode => format!("Mode {}", mode as u8),
    }
}

pub(super) fn raster_depth(core: &Snes, layer: usize) -> String {
    let mut depths = Vec::new();

    for state in core.ppu.window_trace.rows() {
        if let Some(depth) = state.bg_mode.bpp(layer)
            && !depths.contains(&depth)
        {
            depths.push(depth);
        }
    }

    if depths.is_empty()
        && let Some(depth) = core.ppu.bg_mode.bpp(layer)
    {
        depths.push(depth);
    }

    if depths.is_empty() {
        "unused".to_string()
    } else {
        depths.sort_unstable();
        depths
            .into_iter()
            .map(|depth| format!("{depth}bpp"))
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

pub(super) fn raster_uses_mode7(core: &Snes) -> bool {
    use crate::ppu::regs::BackgroundMode;

    core.ppu.bg_mode as u8 == BackgroundMode::Mode7 as u8
        || core.ppu.window_trace.uses_mode(BackgroundMode::Mode7)
}

pub(super) fn mode7_bg_layer(core: &Snes, layer: usize) -> bool {
    layer == 0 && raster_uses_mode7(core)
}

pub(super) fn bg_layer_name(core: &Snes, layer: usize) -> String {
    if mode7_bg_layer(core, layer) {
        "BG1 / M7".to_string()
    } else {
        format!("BG{}", layer + 1)
    }
}

pub(super) fn bg_palette_colors(
    core: &Snes,
    layer: usize,
    bpp: u8,
    palette: usize,
) -> Vec<(u8, u8, u8)> {
    let (base, count) = core.ppu.bg_palette(layer, bpp, palette);
    (0..count)
        .map(|i| crate::ppu::cgram::rgb(core.ppu.cgram.word((base + i) as u8)))
        .collect()
}

pub(super) fn tileset(core: &Snes, layer: usize, color: usize) -> Option<Tileset> {
    let vram: Arc<str> = "vram".into();

    if mode7_bg_layer(core, layer) {
        return Some(mode7_tileset(&core.ppu));
    }

    let data = core.ppu.tileset(layer)?;

    let palette = core.ppu.bg_palette_index(layer, data.bpp, color);
    let colors = self::bg_palette_colors(core, layer, data.bpp, palette);

    let tiles = data
        .tiles
        .iter()
        .enumerate()
        .map(|(i, tile)| TilesetTile {
            address: MemoryAddress {
                region: vram.clone(),
                addr: (data.tile_addr(i) * 2) & 0xFFFF,
            },
            details: vec![],
            pixels: tile
                .decode(data.bpp)
                .as_flattened()
                .iter()
                .map(|&index| rgba(colors[index as usize], index != 0))
                .collect(),
        })
        .collect();

    Some(Tileset {
        tile_width: 8,
        tile_height: 8,
        bpp: data.bpp,
        tile_bytes: data.tile_bytes(),
        tiles,
    })
}

pub(super) fn mode7_colors(ppu: &crate::ppu::Ppu) -> Vec<(u8, u8, u8)> {
    use crate::ppu::regs::ColorSource;

    let direct_color = matches!(ppu.color_math.color_source, ColorSource::Direct);
    (0..=u8::MAX)
        .map(|index| {
            if direct_color {
                crate::ppu::cgram::rgb(crate::ppu::cgram::direct_color(0, index))
            } else {
                crate::ppu::cgram::rgb(ppu.cgram.word(index))
            }
        })
        .collect()
}

pub(super) fn mode7_tile_pixels(ppu: &crate::ppu::Ppu, tile: u8) -> [[u8; 8]; 8] {
    std::array::from_fn(|y| {
        std::array::from_fn(|x| {
            crate::ppu::mode7::mode7_character_pixel(&ppu.vram, tile, x as u16, y as u16)
        })
    })
}

pub(super) fn mode7_tileset(ppu: &crate::ppu::Ppu) -> Tileset {
    let vram: Arc<str> = "vram".into();

    let colors = mode7_colors(ppu);
    let tiles = (0..=u8::MAX)
        .map(|tile| TilesetTile {
            address: MemoryAddress {
                region: vram.clone(),
                addr: ((tile as u32) << 7) | 1,
            },
            details: vec![Field::new("Storage", "Interleaved VRAM high bytes")],
            pixels: mode7_tile_pixels(ppu, tile)
                .as_flattened()
                .iter()
                .map(|&index| rgba(colors[index as usize], index != 0))
                .collect(),
        })
        .collect();

    Tileset {
        tile_width: 8,
        tile_height: 8,
        bpp: 8,
        tile_bytes: 64,
        tiles,
    }
}

pub(super) fn object_pixels(
    width: usize,
    height: usize,
    flip_h: bool,
    flip_v: bool,
    colors: &[(u8, u8, u8)],
    mut tile: impl FnMut(usize, usize) -> [[u8; 8]; 8],
) -> Vec<[u8; 4]> {
    let mut pixels = vec![[0; 4]; width * height];
    for ty in 0..height / 8 {
        for tx in 0..width / 8 {
            for (py, line) in tile(tx, ty).iter().enumerate() {
                for (px, &color) in line.iter().enumerate() {
                    let mut x = tx * 8 + px;
                    let mut y = ty * 8 + py;

                    if flip_h {
                        x = width - 1 - x;
                    }

                    if flip_v {
                        y = height - 1 - y;
                    }

                    pixels[y * width + x] = rgba(colors[color as usize], color != 0);
                }
            }
        }
    }
    pixels
}

pub(super) fn tilemap_window_overlays(core: &Snes, layer: usize) -> Vec<TilemapOverlay> {
    let fallback = crate::ppu::window::WindowDebugState {
        windows: core.ppu.windows,
        main_screen: core.ppu.main_screen_window,
        sub_screen: core.ppu.sub_screen_window,
        bg_mode: core.ppu.bg_mode,
        extbg: core.ppu.extbg,
    };
    let trace = core.ppu.window_trace.rows();
    let mut bands: [Vec<TilemapOverlayBand>; 2] = std::array::from_fn(|_| Vec::new());
    let mut descriptor = [None; 2];
    let mut mixed = [false; 2];

    for y in 0..crate::ppu::SCREEN_HEIGHT as usize {
        let state = trace.get(y).copied().unwrap_or(fallback);
        let (settings, bounds) = state.windows.row_mask(layer as u8).settings();
        let main = state.main_screen.enabled(layer);
        let sub = state.sub_screen.enabled(layer);

        for window in 0..2 {
            let (enabled, inverted) = settings[window];
            if !enabled {
                continue;
            }

            let current_descriptor = (inverted, main, sub);
            if descriptor[window].is_some_and(|value| value != current_descriptor) {
                mixed[window] = true;
            }
            descriptor[window] = Some(current_descriptor);

            let (left, right) = bounds[window];
            let width = if left <= right {
                (right - left) as u32 + 1
            } else {
                0
            };
            let band = TilemapOverlayBand {
                x: left as u32,
                y: y as u32,
                width,
                height: 1,
                inverted,
                active: main || sub,
            };

            if let Some(previous) = bands[window].last_mut()
                && previous.y + previous.height == band.y
                && previous.x == band.x
                && previous.width == band.width
                && previous.inverted == band.inverted
                && previous.active == band.active
            {
                previous.height += 1;
            } else {
                bands[window].push(band);
            }
        }
    }

    bands
        .into_iter()
        .enumerate()
        .filter_map(|(window, bands)| {
            let (inverted, main, sub) = descriptor[window]?;
            let label = if mixed[window] {
                format!("W{} VAR", window + 1)
            } else {
                let screen = match (main, sub) {
                    (true, true) => "M/S",
                    (true, false) => "M",
                    (false, true) => "S",
                    (false, false) => "off",
                };
                let inversion = if inverted { " INV" } else { "" };
                format!("W{} {screen}{inversion}", window + 1)
            };

            Some(TilemapOverlay {
                label: label.into(),
                slot: window,
                bands,
                outline_bands: None,
            })
        })
        .collect()
}

pub(super) fn tilemap(core: &Snes, layer: usize) -> Option<Tilemap> {
    let vram: Arc<str> = "vram".into();

    if mode7_bg_layer(core, layer) {
        return Some(mode7_tilemap(&core.ppu));
    }

    let data = core.ppu.tilemap(layer)?;
    let bpp = core.ppu.bg_mode.bpp(layer)?;

    let palettes: Vec<Vec<(u8, u8, u8)>> = (0..8)
        .map(|palette| self::bg_palette_colors(core, layer, bpp, palette))
        .collect();

    let size = data.tile_px as usize;

    let mut decoded: Vec<Option<[[u8; 8]; 8]>> = vec![None; 1024];
    let mut decode = |index: u16| {
        let index = index & 0x3FF;
        *decoded[index as usize].get_or_insert_with(|| {
            core.ppu
                .bg_tile(layer, index)
                .unwrap_or_default()
                .decode(bpp)
        })
    };

    let mut entries = Vec::with_capacity(data.entries.len());
    for (i, entry) in data.entries.iter().enumerate() {
        let (x, y) = (i as u32 % data.width, i as u32 / data.width);
        let colors = &palettes[entry.palette_selection as usize];

        let pixels =
            self::object_pixels(size, size, entry.flip_h, entry.flip_v, colors, |tx, ty| {
                decode(entry.tile_index + tx as u16 + ty as u16 * 16)
            });

        entries.push(TilemapEntry {
            address: MemoryAddress {
                region: vram.clone(),
                addr: (data.entry_addr(x, y) * 2) & 0xFFFF,
            },
            details: vec![
                Field::new("Tile", hex(entry.tile_index as u32, 12)),
                Field::new("Raw", hex(entry.raw as u32, 16)),
                Field::new("Palette", entry.palette_selection.to_string()),
                Field::new(
                    "Priority",
                    if entry.priority == crate::ppu::regs::Priority::High {
                        "High"
                    } else {
                        "Normal"
                    },
                ),
                Field::new("Flip", format!("H={} V={}", entry.flip_h, entry.flip_v)),
            ],
            pixels,
        });
    }

    Some(Tilemap {
        width: data.width as usize,
        height: data.height as usize,
        tile_px: data.tile_px as u8,
        entries,
        viewport: Some(TilemapViewport {
            transform: None,
            x: data.scroll_x,
            y: data.scroll_y,
            width: crate::ppu::SCREEN_WIDTH,
            height: crate::ppu::SCREEN_HEIGHT,
        }),
    })
}

pub(super) fn mode7_tilemap(ppu: &crate::ppu::Ppu) -> Tilemap {
    let vram: Arc<str> = "vram".into();

    const MAP_TILES: usize = 128;

    let colors = mode7_colors(ppu);
    let decoded: Vec<_> = (0..=u8::MAX)
        .map(|tile| mode7_tile_pixels(ppu, tile))
        .collect();
    let mut entries = Vec::with_capacity(MAP_TILES * MAP_TILES);

    for address in 0..MAP_TILES * MAP_TILES {
        let tile = ppu.vram.word(address as u16) as u8;
        let pixels = decoded[tile as usize]
            .as_flattened()
            .iter()
            .map(|&raw| rgba(colors[raw as usize], raw != 0))
            .collect();

        entries.push(TilemapEntry {
            address: MemoryAddress {
                region: vram.clone(),
                addr: address as u32 * 2,
            },
            details: vec![Field::new("Tile", hex(tile as u32, 8))],
            pixels,
        });
    }

    Tilemap {
        width: MAP_TILES,
        height: MAP_TILES,
        tile_px: 8,
        entries,
        viewport: None,
    }
}

pub(super) fn sprites(core: &Snes) -> Sprites {
    let palettes: Vec<Vec<(u8, u8, u8)>> = (0..8)
        .map(|palette| {
            core.ppu
                .obj_palette(palette)
                .into_iter()
                .map(crate::ppu::cgram::rgb)
                .collect::<Vec<_>>()
        })
        .collect();

    let entries = core
        .ppu
        .sprites()
        .into_iter()
        .map(|sprite| {
            let pixels = self::object_pixels(
                sprite.width as usize,
                sprite.height as usize,
                sprite.flip_h,
                sprite.flip_v,
                &palettes[sprite.palette as usize],
                |tx, ty| {
                    core.ppu
                        .obj_tile(sprite.sub_tile(tx as u32, ty as u32))
                        .decode(4)
                },
            );

            SpriteEntry {
                x: sprite.x,
                y: sprite.y as i32,
                width: sprite.width,
                height: sprite.height,
                details: vec![
                    Field::new("Tile", hex(sprite.tile as u32, 12)),
                    Field::new("VRAM", hex(sprite.tile_addr, 16)),
                    Field::new("Palette", sprite.palette.to_string()),
                    Field::new("Priority", sprite.priority.to_string()),
                    Field::new("Flip", format!("H={} V={}", sprite.flip_h, sprite.flip_v)),
                ],
                pixels,
            }
        })
        .collect();

    Sprites { entries }
}

impl SnesDebug {
    pub(super) fn palette(&self) -> PaletteView {
        let palettes = (0..16)
            .map(|p| {
                self.core
                    .ppu
                    .palette(p)
                    .into_iter()
                    .map(|word| PaletteColor {
                        raw: word as u32,
                        rgb: crate::ppu::cgram::rgb(word),
                    })
                    .collect()
            })
            .collect();

        PaletteView {
            palettes,
            meta: format!("CGADD {}", hex(self.core.ppu.cgram.word_addr() as u32, 8)).into(),
        }
    }

    fn palette_key(&self) -> u64 {
        let cgram = &self.core.ppu.cgram;
        mix([cgram.generation(), cgram.word_addr() as u64])
    }

    fn tiles_key(&self) -> u64 {
        let ppu = &self.core.ppu;
        let mut hasher = DefaultHasher::new();
        hasher.write_u64(ppu.vram.generation());
        hasher.write_u64(ppu.cgram.generation());
        hasher.write_u8(ppu.bg_mode as u8);
        hasher.write_u16(ppu.window_trace.background_mode_mask());
        hasher.write_u8(ppu.extbg as u8);
        hasher.write_u8(ppu.color_math.color_source as u8);
        hasher.write_u8(ppu.bg3_priority as u8);

        for layer in &ppu.bg_layers {
            hasher.write_u32(layer.tileset_addr);
            hasher.write_u32(layer.tilemap_addr);
            hasher.write_u8(layer.horizontal_tilemap_count as u8);
            hasher.write_u8(layer.vertical_tilemap_count as u8);
            hasher.write_u8(layer.tile_size as u8);
        }

        hasher.write_u32(ppu.obj.tileset_addr);
        hasher.write_u32(ppu.obj.name_offset);
        hasher.write_u8(ppu.obj.sprite_size as u8);
        hasher.finish()
    }

    fn window_key(&self) -> u64 {
        use crate::ppu::regs::{W12SEL, W34SEL, WBGLOG, WH0, WOBJLOG, WOBJSEL};

        let ppu = &self.core.ppu;
        let mut hasher = DefaultHasher::new();
        for register in [W12SEL, W34SEL, WOBJSEL] {
            hasher.write_u8(ppu.windows.selector(register).raw());
        }

        for register in WH0..=WH0 + 3 {
            hasher.write_u8(ppu.windows.position(register));
        }

        for register in WBGLOG..=WOBJLOG {
            hasher.write_u8(ppu.windows.logic(register));
        }

        hasher.write_u8(ppu.main_screen_window.raw());
        hasher.write_u8(ppu.sub_screen_window.raw());
        hasher.write_u64(ppu.window_trace.generation());
        hasher.finish()
    }

    fn sprites_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write_u64(self.tiles_key());
        for i in 0..544 {
            hasher.write_u8(self.core.ppu.oam.byte(i));
        }

        hasher.finish()
    }
}

fn mix(parts: impl IntoIterator<Item = u64>) -> u64 {
    let mut hasher = DefaultHasher::new();
    for part in parts {
        hasher.write_u64(part);
    }

    hasher.finish()
}

fn rgba((r, g, b): (u8, u8, u8), opaque: bool) -> [u8; 4] {
    [r, g, b, if opaque { 255 } else { 0 }]
}

const PALETTE: &str = "snes.palette";
const SPRITES: &str = "snes.objects";
const GSU_BITMAP: &str = "snes.gsu.bitmap";
const TILES_PREFIX: &str = "snes.tiles.";
const MAP_PREFIX: &str = "snes.map.";

fn bg_index(id: &VideoResourceId, prefix: &str) -> Option<usize> {
    id.as_str()
        .strip_prefix(prefix)?
        .parse::<usize>()
        .ok()
        .filter(|&i| i < BG_LAYERS)
}

impl VideoDebug for SnesDebug {
    fn resources(&self) -> Vec<VideoResource> {
        use VideoResourceKind::*;
        let mut resources = vec![
            VideoResource::new(PALETTE, "CGRAM", Palette),
            VideoResource::new(SPRITES, "Objects", Sprites),
        ];

        for i in 0..BG_LAYERS {
            let name = bg_layer_name(&self.core, i);
            for (prefix, kind) in [(TILES_PREFIX, Tileset), (MAP_PREFIX, Tilemap)] {
                resources.push(VideoResource::new(
                    format!("{prefix}{i}"),
                    name.clone(),
                    kind,
                ));
            }
        }

        if self.has_gsu() {
            resources.push(VideoResource::new(GSU_BITMAP, "GSU Bitmap", Bitmap));
        }

        resources
    }

    fn revision(&self, resource: &VideoResourceId) -> Option<VideoRevision> {
        let pixels = match resource.as_str() {
            PALETTE => self.palette_key(),
            SPRITES => self.sprites_key(),
            GSU_BITMAP if self.has_gsu() => {
                let fx = &self.core.superfx;
                let (base, mode, plot) =
                    fx.stopped_screen
                        .unwrap_or((fx.scbr, fx.scmr.raw(), fx.por.raw()));

                let mut hash = DefaultHasher::new();
                hash.write(&self.core.mmu.cartridge.sram);
                hash.write(&[base, mode, plot]);
                hash.write_u64(self.palette_key());
                hash.finish()
            }
            _ if bg_index(resource, TILES_PREFIX).is_some() => self.tiles_key(),
            _ if bg_index(resource, MAP_PREFIX).is_some() => self.tiles_key(),
            _ => return None,
        };

        let overlay = match bg_index(resource, MAP_PREFIX) {
            Some(layer) => {
                let mut hash = DefaultHasher::new();
                hash.write_u64(self.window_key());
                if let Some(header) = self.core.ppu.tilemap_header(layer) {
                    hash.write_u32(header.scroll_x);
                    hash.write_u32(header.scroll_y);
                }

                hash.finish()
            }
            None => 0,
        };

        Some(VideoRevision {
            pixels,
            metadata: pixels,
            overlay,
        })
    }

    fn palette(&self, resource: &VideoResourceId) -> Option<PaletteView> {
        (resource.as_str() == PALETTE).then(|| SnesDebug::palette(self))
    }

    fn tileset(
        &self,
        resource: &VideoResourceId,
        palette: Option<&debugger_model::PaletteSelection>,
    ) -> Option<Tileset> {
        let color = palette
            .filter(|p| p.resource.as_str() == PALETTE)
            .map_or(0, |p| p.color);
        tileset(&self.core, bg_index(resource, TILES_PREFIX)?, color)
    }

    fn tilemap(&self, resource: &VideoResourceId) -> Option<Tilemap> {
        tilemap(&self.core, bg_index(resource, MAP_PREFIX)?)
    }

    fn tilemap_overlays(&self, resource: &VideoResourceId) -> Vec<TilemapOverlay> {
        let Some(layer) = bg_index(resource, MAP_PREFIX) else {
            return vec![];
        };

        if mode7_bg_layer(&self.core, layer) {
            return vec![];
        }

        tilemap_window_overlays(&self.core, layer)
    }

    fn tilemap_viewport(&self, resource: &VideoResourceId) -> Option<TilemapViewport> {
        let layer = bg_index(resource, MAP_PREFIX)?;
        if mode7_bg_layer(&self.core, layer) {
            return None;
        }

        let header = self.core.ppu.tilemap_header(layer)?;
        Some(TilemapViewport {
            x: header.scroll_x,
            y: header.scroll_y,
            width: crate::ppu::SCREEN_WIDTH,
            height: crate::ppu::SCREEN_HEIGHT,
            transform: None,
        })
    }

    fn sprites(&self, resource: &VideoResourceId) -> Option<Sprites> {
        (resource.as_str() == SPRITES).then(|| sprites(&self.core))
    }

    fn bitmap(&self, resource: &VideoResourceId) -> Option<BitmapImage> {
        (resource.as_str() == GSU_BITMAP && self.has_gsu())
            .then(|| superfx::bitmap_image(&self.core))
    }
}
