use super::*;

pub(super) fn ppu_view(core: &Snes) -> PpuView {
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
        PpuProperty::new("BG mode", mode).accented(),
        PpuProperty::new("BG3 priority", bg3_priority),
    ];

    if ranges.len() > 1 {
        const SHOWN: usize = 6;
        let extra = ranges.len().saturating_sub(SHOWN);
        for (i, range) in ranges.iter().take(SHOWN).enumerate() {
            let label = if i == 0 { "Raster" } else { "" };
            properties.push(PpuProperty::new(label, range.clone()));
        }
        if extra > 0 {
            properties.push(PpuProperty::new("", format!("+ {extra} more")));
        }
    }

    let groups = core
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

            PpuGroup::new(
                format!("ppu-bg{}", i + 1),
                name,
                vec![
                    PpuProperty::new("Tileset", tileset.clone()),
                    PpuProperty::new("Tilemap", tilemap),
                    PpuProperty::new("Tile size", tile_size),
                    PpuProperty::new("Depth", depth),
                ],
            )
            .summary(tileset)
        })
        .collect();

    PpuView { properties, groups }
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
            addr: data.tile_addr(i),
            pixels: tile
                .decode(data.bpp)
                .as_flattened()
                .iter()
                .map(|&index| colors[index as usize])
                .collect(),
        })
        .collect();

    Some(Tileset {
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
    let colors = mode7_colors(ppu);
    let tiles = (0..=u8::MAX)
        .map(|tile| TilesetTile {
            addr: (tile as u32) << 6,
            pixels: mode7_tile_pixels(ppu, tile)
                .as_flattened()
                .iter()
                .map(|&index| colors[index as usize])
                .collect(),
        })
        .collect();

    Tileset {
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
) -> Vec<(u8, u8, u8)> {
    let mut pixels = vec![(0, 0, 0); width * height];
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

                    pixels[y * width + x] = colors[color as usize];
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
            })
        })
        .collect()
}

pub(super) fn tilemap(core: &Snes, layer: usize) -> Option<Tilemap> {
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
            addr: data.entry_addr(x, y),
            raw: entry.raw,
            tile_index: entry.tile_index,
            palette: entry.palette_selection,
            priority: entry.priority == crate::ppu::regs::Priority::High,
            flip_h: entry.flip_h,
            flip_v: entry.flip_v,
            pixels,
        });
    }

    Some(Tilemap {
        width: data.width as usize,
        height: data.height as usize,
        tile_px: data.tile_px as u8,
        entries,
        viewport: Some(TilemapViewport {
            x: data.scroll_x,
            y: data.scroll_y,
            width: crate::ppu::SCREEN_WIDTH,
            height: crate::ppu::SCREEN_HEIGHT,
        }),
    })
}

pub(super) fn mode7_tilemap(ppu: &crate::ppu::Ppu) -> Tilemap {
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
            .map(|&raw| colors[raw as usize])
            .collect();
        entries.push(TilemapEntry {
            addr: address as u32,
            raw: tile as u16,
            tile_index: tile as u16,
            palette: 0,
            priority: false,
            flip_h: false,
            flip_v: false,
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
            let mut colors: Vec<_> = core
                .ppu
                .obj_palette(palette)
                .into_iter()
                .map(crate::ppu::cgram::rgb)
                .collect();
            // Color 0 is transparent; render it black.
            colors[0] = (0, 0, 0);
            colors
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
                y: sprite.y,
                width: sprite.width,
                height: sprite.height,
                tile: sprite.tile,
                tile_addr: sprite.tile_addr,
                palette: sprite.palette,
                priority: sprite.priority,
                flip_h: sprite.flip_h,
                flip_v: sprite.flip_v,
                pixels,
            }
        })
        .collect();

    let size = core.ppu.obj.sprite_size;

    Sprites {
        small: size.small(),
        large: size.large(),
        entries,
    }
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
                        raw: word,
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

    pub(super) fn layers(&self) -> Vec<TilesetLayer> {
        let mut layers: Vec<TilesetLayer> = (0..4)
            .map(|i| TilesetLayer {
                name: bg_layer_name(&self.core, i).into(),
                enabled: self.core.ppu.bg_mode.bpp(i).is_some() || mode7_bg_layer(&self.core, i),
            })
            .collect();
        if self.has_gsu() {
            layers.push(TilesetLayer {
                name: "GSU".into(),
                enabled: true,
            });
        }
        layers
    }

    pub(super) fn generations(&self) -> Option<VideoGenerations> {
        use std::hash::{DefaultHasher, Hasher};

        fn mix(parts: impl IntoIterator<Item = u64>) -> u64 {
            let mut hasher = DefaultHasher::new();
            for part in parts {
                hasher.write_u64(part);
            }
            hasher.finish()
        }

        let ppu = &self.core.ppu;
        let vram = ppu.vram.generation();
        let cgram = ppu.cgram.generation();

        let regs = {
            let mut hasher = DefaultHasher::new();
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
        };

        let window_regs = {
            use crate::ppu::regs::{W12SEL, W34SEL, WBGLOG, WH0, WOBJLOG, WOBJSEL};

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
            hasher.finish()
        };

        let oam = {
            let mut hasher = DefaultHasher::new();
            for i in 0..128 {
                hasher.write_u8(ppu.oam.byte(i * 4 + 2));
                hasher.write_u8(ppu.oam.byte(i * 4 + 3));
            }
            for i in 0..32 {
                hasher.write_u8(ppu.oam.byte(512 + i) & 0b1010_1010);
            }
            hasher.finish()
        };

        Some(VideoGenerations {
            palette: mix([cgram, ppu.cgram.word_addr() as u64]),
            tileset: mix([vram, cgram, regs]),
            tilemap: mix([vram, cgram, regs]),
            tilemap_overlay: mix([window_regs, ppu.window_trace.generation()]),
            sprites: mix([vram, cgram, regs, oam]),
        })
    }
}
