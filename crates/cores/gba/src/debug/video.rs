use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use debugger_model::{
    BitmapImage, BitmapSource, Field, FlagView, MemoryAddress, PaletteColor, PaletteSelection,
    PaletteView, Row, Section, SpriteEntry, Sprites, StructuredView, Tilemap, TilemapEntry,
    TilemapOverlay, TilemapOverlayBand, TilemapViewport, Tileset, TilesetTile, VideoDebug,
    VideoResource, VideoResourceId, VideoResourceKind, VideoRevision, panel,
};
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelGroupLayout, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

use super::window_trace::WindowDebugRow;
use super::{GbaDebug, field};
use crate::ppu::mosaic::MosaicSize;
use crate::ppu::regs::{BackgroundKind, BackgroundMode, SpecialEffect, SpriteMapping};
use crate::ppu::renderer::{render_bitmap_line, render_sprite_line, sprite_texel};
use crate::ppu::sprite::{self, OBJ_PALETTE_BASE, ObjMode};
use crate::ppu::tile::{ColorDepth, Tile};
use crate::ppu::window::WindowRegion;
use crate::ppu::{BitmapLayout, VRAM_BASE, rgba};
use crate::{Gba, SCREEN_HEIGHT, SCREEN_WIDTH};

const PALETTE: &str = "gba.palette";
const OBJ_PALETTE: &str = "gba.palette.obj";
const SPRITES: &str = "gba.objects";

impl WindowRegion {
    const NAMES: [&str; 4] = ["WIN0", "WIN1", "OBJ", "OUT"];
    const OBJ_SLOT: usize = 2;
    const OUTSIDE_SLOT: usize = 3;

    fn slot(self) -> usize {
        match self {
            Self::Rectangle(index) => index,
            Self::Obj => Self::OBJ_SLOT,
            Self::Outside => Self::OUTSIDE_SLOT,
        }
    }
}

fn background_row(core: &Gba, index: usize) -> Row {
    let display = core.ppu.dispctrl;
    let control = core.ppu.bgcnt[index];
    let kind = display.mode().background_kind(index);
    let enabled = display.bg_enabled(index);
    let priority = control.priority();
    let layout = core
        .ppu
        .bitmap_layout()
        .filter(|_| kind == BackgroundKind::Bitmap);
    let (size, colors, addresses) = match layout {
        Some(layout) => (
            Some((layout.width, layout.height)),
            if layout.bytes_per_pixel == 1 {
                ColorDepth::Bpp8.to_string()
            } else {
                "15-bit direct color".to_string()
            },
            vec![field("Framebuffer", format!("{:08X}", layout.address()))],
        ),
        None => (
            control.dimensions(kind),
            control.effective_color_depth(kind).to_string(),
            vec![
                field("Tileset base", format!("{:08X}", control.tileset_address())),
                field("Tilemap base", format!("{:08X}", control.tilemap_address())),
            ],
        ),
    };
    let dimensions = size.map_or_else(
        || "N/A".to_string(),
        |(width, height)| format!("{width} × {height}"),
    );
    let summary = format!("{kind} · {dimensions} · prio {priority}");

    let mut flags = vec![
        FlagView::new("Enabled", enabled),
        FlagView::new("Mosaic", control.mosaic()),
    ];
    if kind == BackgroundKind::Affine {
        flags.push(FlagView::new("Wraparound", control.wraparound()));
    }
    let mut rows = vec![
        field("Type", kind.to_string()),
        field("Priority", priority.to_string()),
        field("Dimensions", dimensions),
        field("Colors", colors),
    ];
    rows.extend(addresses);
    if kind == BackgroundKind::Text {
        rows.push(field(
            "X offset",
            core.ppu.bghofs[index].offset().to_string(),
        ));
        rows.push(field(
            "Y offset",
            core.ppu.bgvofs[index].offset().to_string(),
        ));
    }
    rows.push(Row::Flags { flags, note: None });

    Row::Group {
        id: format!("bg{index}").into(),
        title: format!("BG{index}").into(),
        summary: summary.into(),
        active: enabled,
        rows,
    }
}

const BG_LAYERS: [(&str, &str); 4] = [
    ("gba.bg0", "BG0"),
    ("gba.bg1", "BG1"),
    ("gba.bg2", "BG2"),
    ("gba.bg3", "BG3"),
];

const BG_TILESETS: [(&str, &str); 4] = [
    ("gba.tiles.0", "BG0"),
    ("gba.tiles.1", "BG1"),
    ("gba.tiles.2", "BG2"),
    ("gba.tiles.3", "BG3"),
];

fn layer_index(table: &[(&str, &str); 4], id: &str) -> Option<usize> {
    table.iter().position(|&(candidate, _)| candidate == id)
}

fn background_tileset(core: &Gba, index: usize, color: usize) -> Option<Tileset> {
    let ppu = &core.ppu;
    let control = ppu.bgcnt[index];
    let kind = ppu.dispctrl.mode().background_kind(index);
    let limit = kind.tile_count()?;
    let depth = control.effective_color_depth(kind);
    let tile_bytes = depth.tile_size();
    let base = control.tileset_offset();
    let tile_count = limit.min((0x10000 - base) / tile_bytes);
    let palette_base = depth.palette_base((color / 16) % 16);
    let tiles = (0..tile_count)
        .map(|index| {
            let offset = base + (index * tile_bytes);
            let tile = Tile {
                data: &ppu.vram[offset..(offset + tile_bytes)],
                depth,
            };
            let pixels = (0..64)
                .map(|pixel| {
                    let color = tile.pixel(pixel % 8, pixel / 8) as usize;
                    if color == 0 {
                        [0; 4]
                    } else {
                        rgba(ppu.palette_color(palette_base + color))
                    }
                })
                .collect();
            TilesetTile {
                address: MemoryAddress {
                    region: "gba.vram".into(),
                    addr: VRAM_BASE + (offset as u32),
                },
                pixels,
                details: vec![],
            }
        })
        .collect();

    Some(Tileset {
        bpp: depth.bits(),
        tile_bytes: tile_bytes as u32,
        tile_width: 8,
        tile_height: 8,
        tiles,
    })
}

fn background_viewport(core: &Gba, index: usize) -> Option<TilemapViewport> {
    (core.ppu.dispctrl.mode().background_kind(index) == BackgroundKind::Text).then(|| {
        TilemapViewport {
            x: u32::from(core.ppu.bghofs[index].offset()),
            y: u32::from(core.ppu.bgvofs[index].offset()),
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
            transform: None,
        }
    })
}

fn background_tilemap(core: &Gba, index: usize) -> Option<Tilemap> {
    let ppu = &core.ppu;
    let control = ppu.bgcnt[index];
    let kind = ppu.dispctrl.mode().background_kind(index);
    let (width, height) = control.dimensions(kind)?;
    let depth = control.effective_color_depth(kind);
    let map_width = width / 8;
    let map_height = height / 8;
    let mut entries = Vec::with_capacity(map_width * map_height);

    for y in 0..map_height {
        for x in 0..map_width {
            let (entry_addr, entry) = control.map_entry(kind, &ppu.vram, x, y, width)?;
            let tile_addr = control.tile_offset(entry.tile_number() as usize, depth);
            let palette_base = entry.palette_base(depth);
            let mut pixels = vec![[0; 4]; 64];
            if let Some(data) = ppu.vram.get(tile_addr..(tile_addr + depth.tile_size())) {
                let tile = Tile { data, depth };
                for (pixel, dst) in pixels.iter_mut().enumerate() {
                    let color = entry.pixel(&tile, pixel % 8, pixel / 8) as usize;
                    if color != 0 {
                        *dst = rgba(ppu.palette_color(palette_base + color));
                    }
                }
            }

            let mut details = vec![
                Field::new("Tile", format!("{:03X}", entry.tile_number())),
                Field::new(
                    "Tile address",
                    format!("{:08X}", VRAM_BASE + (tile_addr as u32)),
                ),
            ];
            if kind == BackgroundKind::Text {
                details.push(Field::new("Raw", format!("{:04X}", entry.raw())));
                details.push(Field::new(
                    "Flip",
                    format!("H={} V={}", entry.horizontal_flip(), entry.vertical_flip()),
                ));
                if matches!(depth, ColorDepth::Bpp4) {
                    details.push(Field::new("Palette", entry.palette_number().to_string()));
                }
            }
            entries.push(TilemapEntry {
                address: MemoryAddress {
                    region: "gba.vram".into(),
                    addr: VRAM_BASE + (entry_addr as u32),
                },
                pixels,
                details,
            });
        }
    }

    Some(Tilemap {
        width: map_width,
        height: map_height,
        tile_px: 8,
        entries,
        viewport: background_viewport(core, index),
    })
}

fn tilemap_window_overlays(core: &Gba, layer: usize) -> Vec<TilemapOverlay> {
    let Some(viewport) = background_viewport(core, layer) else {
        return Vec::new();
    };
    let ppu = &core.ppu;
    let Some((map_width, map_height)) = ppu.bgcnt[layer].dimensions(BackgroundKind::Text) else {
        return Vec::new();
    };
    let (map_width, map_height) = (map_width as u32, map_height as u32);
    let trace = ppu.window_trace.rows();
    let mut bands: [Vec<TilemapOverlayBand>; 4] = std::array::from_fn(|_| Vec::new());
    let mut outlines: [Vec<TilemapOverlayBand>; 4] = std::array::from_fn(|_| Vec::new());
    let mut previous = HashMap::new();
    let mut next = HashMap::new();

    for y in 0..SCREEN_HEIGHT as usize {
        let recorded = trace.get(y).copied().flatten();
        let row = recorded.unwrap_or_else(|| {
            let mut obj_window = [false; SCREEN_WIDTH as usize];
            if ppu.dispctrl.enable_obj_window() {
                let mut sprites = [None; SCREEN_WIDTH as usize];
                render_sprite_line(
                    ppu.dispctrl,
                    ppu.mosaic.obj_size(),
                    ppu.memory_view(),
                    y,
                    &mut sprites,
                    &mut obj_window,
                );
            }
            WindowDebugRow::capture(ppu, obj_window)
        });
        next.clear();
        if row.display.mode().background_kind(layer) != BackgroundKind::Text {
            previous.clear();
            continue;
        }
        let pixels: [_; SCREEN_WIDTH as usize] = std::array::from_fn(|x| {
            let region = row.windows.region(x, y, row.obj_window[x])?;
            let masked =
                !row.display.bg_enabled(layer) || !region.mask(row.windows).bg_enabled(layer);
            Some((region.slot(), masked))
        });

        let mut push_band = |slot: usize, start: usize, end: usize, masked: bool, outline: bool| {
            let target = if outline {
                &mut outlines[slot]
            } else {
                &mut bands[slot]
            };
            let band = TilemapOverlayBand {
                x: (u32::from(row.hofs[layer]) + start as u32 + map_width - viewport.x % map_width)
                    % map_width,
                y: (u32::from(row.vofs[layer]) + y as u32 + map_height - viewport.y % map_height)
                    % map_height,
                width: (end - start) as u32,
                height: 1,
                inverted: false,
                active: masked,
            };
            let key = (slot, band.x, band.width, masked, outline);
            let merged = previous.get(&key).copied().filter(|&index: &usize| {
                let last = &target[index];
                last.y + last.height == band.y
            });
            let index = if let Some(index) = merged {
                target[index].height += 1;
                index
            } else {
                let index = target.len();
                target.push(band);
                index
            };
            next.insert(key, index);
        };

        let mut x = 0;
        while x < pixels.len() {
            let start = x;
            let pixel = pixels[x];
            while x < pixels.len() && pixels[x] == pixel {
                x += 1;
            }
            if let Some((slot, masked)) = pixel {
                push_band(slot, start, x, masked, false);
            }
        }
        if row.windows.any_enabled() {
            for slot in 0..WindowRegion::OUTSIDE_SLOT {
                let contains = |x| match slot {
                    WindowRegion::OBJ_SLOT => row.windows.obj_enabled && row.obj_window[x],
                    _ => row.windows.contains(slot, x, y),
                };
                let mut x = 0;
                while x < SCREEN_WIDTH as usize {
                    let start = x;
                    let inside = contains(x);
                    while x < SCREEN_WIDTH as usize && contains(x) == inside {
                        x += 1;
                    }
                    if inside {
                        push_band(slot, start, x, false, true);
                    }
                }
            }
        }
        std::mem::swap(&mut previous, &mut next);
    }
    outlines[WindowRegion::OUTSIDE_SLOT].clone_from(&bands[WindowRegion::OUTSIDE_SLOT]);

    bands
        .into_iter()
        .zip(outlines)
        .enumerate()
        .filter_map(|(slot, (bands, outlines))| {
            if bands.is_empty() && outlines.is_empty() {
                return None;
            }
            Some(TilemapOverlay {
                label: WindowRegion::NAMES[slot].into(),
                slot,
                bands,
                outline_bands: Some(outlines),
            })
        })
        .collect()
}

fn bitmap_bg2(core: &Gba) -> Option<BitmapImage> {
    let control = core.ppu.dispctrl;
    let mode = control.mode();
    let layout = core.ppu.bitmap_layout()?;
    let BitmapLayout {
        width,
        height,
        bytes_per_pixel,
        base,
    } = layout;
    let memory = core.ppu.memory_view();
    let backdrop = core.ppu.palette_color(0);
    let mut pixels = Vec::with_capacity(width * height);
    let mut output = [None; SCREEN_WIDTH as usize];
    for line in 0..height {
        render_bitmap_line(layout, MosaicSize::NONE, memory, line, &mut output);
        pixels.extend(
            output[..width]
                .iter()
                .map(|color| rgba(color.unwrap_or(backdrop))),
        );
    }

    Some(BitmapImage {
        width: width as u32,
        height: height as u32,
        pixels: pixels.into(),
        details: vec![
            Field::new("Mode", mode.to_string()),
            Field::new("Enabled", control.bg2_enabled().to_string()),
            Field::new(
                "Frame",
                if mode == BackgroundMode::Mode3 {
                    "Frame 0".to_string()
                } else {
                    control.frame_select().to_string()
                },
            ),
        ],
        source: Some(BitmapSource {
            address: MemoryAddress {
                region: "gba.vram".into(),
                addr: layout.address(),
            },
            bytes_per_pixel: bytes_per_pixel as u32,
        }),
        indices: (mode == BackgroundMode::Mode4).then(|| {
            core.ppu.vram[base..base + width * height]
                .iter()
                .map(|&index| u16::from(index))
                .collect()
        }),
    })
}

fn sprites(core: &Gba) -> Sprites {
    let ppu = &core.ppu;
    let mapping = ppu.dispctrl.sprite_mapping();
    let memory = ppu.memory_view();
    let entries = sprite::attributes(memory.oam)
        .map(|attr| {
            let (width, height) = attr.dimensions();
            let mut pixels = vec![[0; 4]; width * height];
            for (row, line) in pixels.chunks_exact_mut(width.max(1)).enumerate() {
                for (col, dst) in line.iter_mut().enumerate() {
                    let (x, y) = attr.flip(col, row);
                    if let Some(color) = sprite_texel(attr, mapping, memory, x, y) {
                        *dst = rgba(color);
                    }
                }
            }

            let x = attr.x_coordinate() as i32;
            let y = attr.y_coordinate() as i32;
            let disabled = !attr.affine() && attr.disable_obj();
            let transform = if attr.affine() {
                Field::new(
                    "Transform",
                    format!(
                        "Affine #{}{}",
                        attr.matrix_index(),
                        if attr.double_size() { " (2x)" } else { "" }
                    ),
                )
            } else {
                Field::new(
                    "Flip",
                    format!(
                        "H={}, V={}",
                        u8::from(attr.horizontal_flip()),
                        u8::from(attr.vertical_flip())
                    ),
                )
            };
            let mode = match attr.mode() {
                ObjMode::Normal => "Normal",
                ObjMode::SemiTransparent => "Semi-transparent",
                ObjMode::ObjWindow => "OBJ window",
                ObjMode::Prohibited => "Prohibited",
            };

            SpriteEntry {
                x: if x >= 256 { x - 512 } else { x },
                y: if y >= SCREEN_HEIGHT as i32 {
                    y - 256
                } else {
                    y
                },
                width: width as u32,
                height: height as u32,
                details: vec![
                    Field::new("Tile", format!("{:03X}", attr.tile_number())),
                    Field::new(
                        "Tile address",
                        format!("{:08X}", VRAM_BASE as usize + attr.tile_addr(mapping, 0, 0)),
                    ),
                    Field::new("Depth", attr.color_depth().to_string()),
                    Field::new("Palette", attr.palette_number().to_string()),
                    Field::new("Priority", attr.priority().to_string()),
                    Field::new("Mode", mode.to_string()),
                    transform,
                    Field::new("Mosaic", attr.mosaic().to_string()),
                    Field::new("Disabled", disabled.to_string()),
                ],
                pixels,
            }
        })
        .collect();

    Sprites { entries }
}

fn layer_names(enabled: impl Fn(usize) -> bool, count: usize) -> String {
    let names = ["BG0", "BG1", "BG2", "BG3", "OBJ", "Backdrop"];
    let selected = names
        .iter()
        .take(count)
        .enumerate()
        .filter_map(|(index, &name)| enabled(index).then_some(name))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        "None".into()
    } else {
        selected.join(", ")
    }
}

fn window_rows(core: &Gba) -> Vec<Row> {
    let ppu = &core.ppu;
    let display = ppu.dispctrl;
    let enabled = [
        display.enable_window0(),
        display.enable_window1(),
        display.enable_obj() && display.enable_obj_window(),
    ];
    let permissions = [
        ppu.winin.raw() as u8,
        (ppu.winin.raw() >> 8) as u8,
        (ppu.winout.raw() >> 8) as u8,
        ppu.winout.raw() as u8,
    ];
    let range = |start: u16, end: u16| {
        let suffix = if start == end {
            " (empty)"
        } else if start > end {
            " (wrap)"
        } else {
            ""
        };
        format!("{start} → {end}{suffix}")
    };
    ["WIN0", "WIN1", "OBJ window", "Outside"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let active = enabled
                .get(index)
                .copied()
                .unwrap_or(enabled.contains(&true));
            let mask = permissions[index];
            let mut rows = vec![
                field(
                    "Visible layers",
                    layer_names(|layer| mask & (1 << layer) != 0, 5),
                ),
                field(
                    "Color effects",
                    if mask & 0x20 != 0 {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                    .into(),
                ),
            ];
            if index < 2 {
                rows.push(field(
                    "X (end excl.)",
                    range(ppu.winh[index].x1(), ppu.winh[index].x2()),
                ));
                rows.push(field(
                    "Y (end excl.)",
                    range(ppu.winv[index].y1(), ppu.winv[index].y2()),
                ));
            }
            Row::Group {
                id: format!("ppu-window-{index}").into(),
                title: name.into(),
                summary: if active { "Enabled" } else { "Disabled" }.into(),
                active,
                rows,
            }
        })
        .collect()
}

pub(super) fn ppu_view(core: &Gba) -> StructuredView {
    let control = core.ppu.dispctrl;
    let status = core.ppu.dispstat;
    let vcount = core.ppu.vcount;
    let blend = core.ppu.bldcnt;
    let bg_mosaic = core.ppu.mosaic.bg_size();
    let obj_mosaic = core.ppu.mosaic.obj_size();
    StructuredView {
        sections: vec![
            Section {
                title: Some("Display".into()),
                rows: vec![
                    field("Background Mode", control.mode().to_string()),
                    field("Frame Select", control.frame_select().to_string()),
                    field("Forced blank", control.forced_blank().to_string()),
                    field(
                        "OBJ mapping",
                        match control.sprite_mapping() {
                            SpriteMapping::OneDimensional => "1D",
                            SpriteMapping::TwoDimensional => "2D",
                        }
                        .into(),
                    ),
                ],
                ..Default::default()
            },
            Section {
                title: Some("Enabled layers".into()),
                rows: vec![Row::Flags {
                    flags: vec![
                        FlagView::new("BG0", control.bg0_enabled()),
                        FlagView::new("BG1", control.bg1_enabled()),
                        FlagView::new("BG2", control.bg2_enabled()),
                        FlagView::new("BG3", control.bg3_enabled()),
                        FlagView::new("OBJ", control.enable_obj()),
                        FlagView::new("WIN0", control.enable_window0()),
                        FlagView::new("WIN1", control.enable_window1()),
                        FlagView::new("OBJ window", control.enable_obj_window()),
                    ],
                    note: None,
                }],
                ..Default::default()
            },
            Section {
                title: Some("Status".into()),
                rows: vec![
                    field("Scanline", vcount.scanline().to_string()),
                    field("VCOUNT Compare", status.vcount_compare().to_string()),
                    field("VCOUNT IRQ", status.vcounter_irq().to_string()),
                ],
                ..Default::default()
            },
            Section {
                title: Some("Blending".into()),
                rows: vec![
                    field(
                        "Mode",
                        match blend.special_effect() {
                            SpecialEffect::None => "None",
                            SpecialEffect::AlphaBlend => "Alpha blend",
                            SpecialEffect::BrightnessIncrease => "Brighten",
                            SpecialEffect::BrightnessDecrease => "Darken",
                        }
                        .into(),
                    ),
                    field(
                        "First targets",
                        layer_names(|layer| blend.first_target(layer), 6),
                    ),
                    field(
                        "Second targets",
                        layer_names(|layer| blend.second_target(layer), 6),
                    ),
                    field(
                        "Alpha (first / second)",
                        format!(
                            "{}/16 · {}/16",
                            core.ppu.bldalpha.eva().min(16),
                            core.ppu.bldalpha.evb().min(16)
                        ),
                    ),
                    field("Brightness", format!("{}/16", core.ppu.bldy.ev().min(16))),
                ],
                ..Default::default()
            },
            Section {
                title: Some("Mosaic".into()),
                rows: vec![
                    field(
                        "BG blocks",
                        format!("{} × {} px", bg_mosaic.width, bg_mosaic.height),
                    ),
                    field(
                        "BG enables",
                        layer_names(|layer| core.ppu.bgcnt[layer].mosaic(), 4),
                    ),
                    field(
                        "OBJ blocks",
                        format!("{} × {} px", obj_mosaic.width, obj_mosaic.height),
                    ),
                ],
                ..Default::default()
            },
            Section {
                title: Some("Windows".into()),
                rows: window_rows(core),
                ..Default::default()
            },
            Section {
                title: Some("Backgrounds".into()),
                rows: (0..4).map(|index| background_row(core, index)).collect(),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

pub(super) fn extend_manifest(manifest: &mut ComponentManifest) {
    let workspace = debugger_model::VIDEO_WORKSPACE;
    manifest
        .workspaces
        .push(WorkspaceDescriptor::new(workspace, "PPU", WorkspaceIcon::Video).mmio_group("PPU"));

    let descriptor = |id, title, kind, location| {
        let refresh = if id == panel::PPU {
            RefreshPolicy::Continuous
        } else {
            RefreshPolicy::OnPause
        };
        PanelDescriptor::new(id, title, kind, location, refresh).tag(workspace)
    };

    manifest
        .insert_after(
            panel::BREAKPOINTS,
            descriptor(
                panel::PALETTE,
                "Palettes",
                PanelKind::Palette,
                DockLocation::Left,
            ),
        )
        .expect("shipped panel id");
    manifest
        .insert_before(
            panel::MMIO,
            descriptor(
                panel::PPU,
                "PPU",
                PanelKind::Structured,
                DockLocation::Right,
            ),
        )
        .expect("shipped panel id");
    manifest
        .insert_before(
            panel::MEMORY,
            descriptor(
                panel::TILESET,
                "Tileset",
                PanelKind::Tileset,
                DockLocation::Center,
            ),
        )
        .expect("shipped panel id");
    manifest
        .insert_after(
            panel::TILESET,
            descriptor(
                panel::TILEMAP,
                "Tilemap",
                PanelKind::Tilemap,
                DockLocation::Center,
            ),
        )
        .expect("shipped panel id");
    manifest
        .insert_after(
            panel::TILEMAP,
            descriptor(
                panel::SPRITES,
                "Sprites",
                PanelKind::Sprites,
                DockLocation::Center,
            ),
        )
        .expect("shipped panel id");

    manifest.compose_in(
        workspace,
        panel::TILESET,
        panel::TILEMAP,
        PanelGroupLayout::EqualColumns,
    );
    manifest.compose_in(
        workspace,
        panel::SPRITES,
        panel::FRAMEBUFFER,
        PanelGroupLayout::EqualColumns,
    );
}

impl VideoDebug for GbaDebug {
    fn resources(&self) -> Vec<VideoResource> {
        let palette = VideoResource::new(PALETTE, "BG", VideoResourceKind::Palette);
        let obj_palette = VideoResource::new(OBJ_PALETTE, "OBJ", VideoResourceKind::Palette);
        let sprites = VideoResource::new(SPRITES, "Objects", VideoResourceKind::Sprites);
        let layers = BG_LAYERS
            .iter()
            .map(|&(id, name)| VideoResource::new(id, name, VideoResourceKind::Tilemap));
        let tilesets = BG_TILESETS
            .iter()
            .map(|&(id, name)| VideoResource::new(id, name, VideoResourceKind::Tileset));

        [palette, obj_palette, sprites]
            .into_iter()
            .chain(layers)
            .chain(tilesets)
            .collect()
    }

    fn palette(&self, resource: &VideoResourceId) -> Option<PaletteView> {
        let base = match resource.as_str() {
            PALETTE => 0,
            OBJ_PALETTE => OBJ_PALETTE_BASE,
            _ => return None,
        };

        Some(PaletteView {
            palettes: (0..16)
                .map(|bank| {
                    (0..16)
                        .map(|color| {
                            let raw = self.core.ppu.palette_color(base + (bank * 16) + color);
                            let [r, g, b, _] = rgba(raw);
                            PaletteColor {
                                raw: raw.into(),
                                rgb: (r, g, b),
                            }
                        })
                        .collect()
                })
                .collect(),
            ..Default::default()
        })
    }

    fn tileset(
        &self,
        resource: &VideoResourceId,
        palette: Option<&PaletteSelection>,
    ) -> Option<Tileset> {
        let index = layer_index(&BG_TILESETS, resource.as_str())?;
        let color = palette
            .filter(|selection| selection.resource.as_str() == PALETTE)
            .map_or(0, |selection| selection.color);
        background_tileset(&self.core, index, color)
    }

    fn sprites(&self, resource: &VideoResourceId) -> Option<Sprites> {
        (resource.as_str() == SPRITES).then(|| sprites(&self.core))
    }

    fn bitmap(&self, resource: &VideoResourceId) -> Option<BitmapImage> {
        (layer_index(&BG_LAYERS, resource.as_str())? == 2)
            .then(|| bitmap_bg2(&self.core))
            .flatten()
    }

    fn tilemap(&self, resource: &VideoResourceId) -> Option<Tilemap> {
        background_tilemap(&self.core, layer_index(&BG_LAYERS, resource.as_str())?)
    }

    fn tilemap_viewport(&self, resource: &VideoResourceId) -> Option<TilemapViewport> {
        background_viewport(&self.core, layer_index(&BG_LAYERS, resource.as_str())?)
    }

    fn tilemap_overlays(&self, resource: &VideoResourceId) -> Vec<TilemapOverlay> {
        layer_index(&BG_LAYERS, resource.as_str())
            .map_or_else(Vec::new, |layer| tilemap_window_overlays(&self.core, layer))
    }

    fn revision(&self, resource: &VideoResourceId) -> Option<VideoRevision> {
        let revisions = self.core.ppu.revisions;
        let layer = layer_index(&BG_LAYERS, resource.as_str());
        let pixels = match resource.as_str() {
            PALETTE | OBJ_PALETTE => revisions.palette,
            SPRITES => {
                let mut hash = DefaultHasher::new();
                revisions.vram.hash(&mut hash);
                revisions.palette.hash(&mut hash);
                revisions.oam.hash(&mut hash);
                self.core.ppu.dispctrl.raw().hash(&mut hash);
                hash.finish()
            }
            id => {
                let index = layer.or_else(|| layer_index(&BG_TILESETS, id))?;
                let mut hash = DefaultHasher::new();
                revisions.vram.hash(&mut hash);
                revisions.palette.hash(&mut hash);
                self.core.ppu.dispctrl.raw().hash(&mut hash);
                self.core.ppu.bgcnt[index].raw().hash(&mut hash);
                hash.finish()
            }
        };

        let overlay = layer.map_or(0, |index| {
            let mut hash = DefaultHasher::new();
            let ppu = &self.core.ppu;
            index.hash(&mut hash);
            ppu.dispctrl.raw().hash(&mut hash);
            ppu.bgcnt[index].raw().hash(&mut hash);
            ppu.winh.map(|value| value.raw()).hash(&mut hash);
            ppu.winv.map(|value| value.raw()).hash(&mut hash);
            ppu.winin.raw().hash(&mut hash);
            ppu.winout.raw().hash(&mut hash);
            ppu.revisions.vram.hash(&mut hash);
            ppu.revisions.oam.hash(&mut hash);
            ppu.window_trace.generation().hash(&mut hash);
            ppu.bghofs[index].raw().hash(&mut hash);
            ppu.bgvofs[index].raw().hash(&mut hash);
            hash.finish()
        });

        Some(VideoRevision {
            pixels,
            metadata: pixels,
            overlay,
        })
    }
}
