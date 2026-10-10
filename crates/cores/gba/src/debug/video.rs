use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use debugger_model::{
    BitmapImage, BitmapSource, Field, FlagView, MemoryAddress, PaletteColor, PaletteSelection,
    PaletteView, Row, Section, SpriteEntry, Sprites, StructuredView, Tilemap, TilemapEntry,
    TilemapViewport, Tileset, TilesetTile, VideoDebug, VideoResource, VideoResourceId,
    VideoResourceKind, VideoRevision, panel,
};
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelGroupLayout, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

use super::{GbaDebug, field};
use crate::ppu::regs::{BackgroundKind, BackgroundMode};
use crate::ppu::sprite::{self, ObjMode};
use crate::ppu::tile::{ColorDepth, Tile};
use crate::ppu::{
    BitmapLayout, VRAM_BASE, bitmap_layout, render_bitmap_scanline, rgba, sprite_row,
};
use crate::{Gba, SCREEN_HEIGHT, SCREEN_WIDTH};

const PALETTE: &str = "gba.palette";
const SPRITES: &str = "gba.objects";

fn background_row(core: &Gba, index: usize) -> Row {
    let display = core.ppu.dispctrl;
    let control = core.ppu.bgcnt[index];
    let kind = display.mode().background_kind(index);
    let enabled = display.bg_enabled(index);
    let priority = control.priority();
    let layout = bitmap_layout(display).filter(|_| kind == BackgroundKind::Bitmap);
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
    let summary = format!("{kind} · {dimensions} · priority {priority}");

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

fn bitmap_bg2(core: &Gba) -> Option<BitmapImage> {
    let control = core.ppu.dispctrl;
    let mode = control.mode();
    let layout = bitmap_layout(control)?;
    let BitmapLayout {
        width,
        height,
        bytes_per_pixel,
        base,
    } = layout;
    let mut pixels = vec![[0; 4]; width * height];
    let memory = core.ppu.memory();
    for (line, output) in pixels.chunks_exact_mut(width).enumerate() {
        render_bitmap_scanline(layout, memory, line, output);
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
    let memory = ppu.memory();
    let entries = sprite::attributes(memory.oam)
        .map(|attr| {
            let (width, height) = attr.dimensions();
            let mut pixels = vec![[0; 4]; width * height];
            for (row, line) in pixels.chunks_exact_mut(width).enumerate() {
                for (col, color) in sprite_row(attr, mapping, memory, row) {
                    line[col] = color;
                }
            }

            let x = attr.x_coordinate() as i32;
            let y = attr.y_coordinate() as i32;
            let disabled = !attr.affine() && attr.disable_obj();
            let transform = if attr.affine() {
                Field::new(
                    "Transform",
                    format!(
                        "Affine #{}, double size {}",
                        attr.rotation_scaling_parameter(),
                        attr.double_size()
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

pub(super) fn ppu_view(core: &Gba) -> StructuredView {
    let control = core.ppu.dispctrl;
    let status = core.ppu.dispstat;
    let vcount = core.ppu.vcount;
    StructuredView {
        sections: vec![
            Section {
                title: Some("Display".into()),
                rows: vec![
                    field("Background Mode", control.mode().to_string()),
                    field("Frame Select", control.frame_select().to_string()),
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
        PanelDescriptor::new(id, title, kind, location, RefreshPolicy::OnPause).tag(workspace)
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
        let palette = VideoResource::new(PALETTE, "Palette", VideoResourceKind::Palette);
        let sprites = VideoResource::new(SPRITES, "Objects", VideoResourceKind::Sprites);
        let layers = BG_LAYERS
            .iter()
            .map(|&(id, name)| VideoResource::new(id, name, VideoResourceKind::Tilemap));
        let tilesets = BG_TILESETS
            .iter()
            .map(|&(id, name)| VideoResource::new(id, name, VideoResourceKind::Tileset));

        [palette, sprites]
            .into_iter()
            .chain(layers)
            .chain(tilesets)
            .collect()
    }

    fn palette(&self, resource: &VideoResourceId) -> Option<PaletteView> {
        if resource.as_str() != PALETTE {
            return None;
        }

        Some(PaletteView {
            palettes: (0..16)
                .map(|bank| {
                    (0..16)
                        .map(|color| {
                            let raw = self.core.ppu.palette_color(bank * 16 + color);
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

    fn revision(&self, resource: &VideoResourceId) -> Option<VideoRevision> {
        let revisions = self.core.ppu.revisions;
        let layer = layer_index(&BG_LAYERS, resource.as_str());
        let pixels = match resource.as_str() {
            PALETTE => revisions.palette,
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
            self.core.ppu.bghofs[index].raw().hash(&mut hash);
            self.core.ppu.bgvofs[index].raw().hash(&mut hash);
            hash.finish()
        });

        Some(VideoRevision {
            pixels,
            metadata: pixels,
            overlay,
        })
    }
}
