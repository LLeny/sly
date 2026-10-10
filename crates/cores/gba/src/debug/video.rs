use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use debugger_model::{
    BitmapImage, BitmapSource, Field, FlagView, MemoryAddress, PaletteColor, PaletteView, Row,
    Section, SpriteEntry, Sprites, StructuredView, VideoDebug, VideoResource, VideoResourceId,
    VideoResourceKind, VideoRevision, panel,
};
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

use super::{GbaDebug, field};
use crate::ppu::regs::{BackgroundKind, BackgroundMode};
use crate::ppu::sprite::{self, ObjMode};
use crate::ppu::tile::ColorDepth;
use crate::ppu::{BitmapLayout, VRAM_BASE, bitmap_layout, render_bg_scanline, rgba, sprite_row};
use crate::{Gba, SCREEN_HEIGHT};

const PALETTE: &str = "gba.palette";
const SPRITES: &str = "gba.objects";

fn background_row(core: &Gba, index: usize) -> Row {
    let display = core.ppu.dispctrl;
    let control = core.ppu.bgcnt[index];
    let kind = display.mode().background_kind(index);
    let enabled = display.enable_bg(index);
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

fn bg_layer(id: &str) -> Option<usize> {
    BG_LAYERS.iter().position(|&(candidate, _)| candidate == id)
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
    for (line, output) in pixels.chunks_exact_mut(width).enumerate() {
        render_bg_scanline(layout, &core.ppu.vram, &core.ppu.palette, line, output);
    }

    Some(BitmapImage {
        width: width as u32,
        height: height as u32,
        pixels: pixels.into(),
        details: vec![
            Field::new("Mode", mode.to_string()),
            Field::new("Enabled", control.enable_bg2().to_string()),
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
    let entries = sprite::attributes(&ppu.oam)
        .map(|attr| {
            let (width, height) = attr.dimensions();
            let mut pixels = vec![[0; 4]; width * height];
            for (row, line) in pixels.chunks_exact_mut(width).enumerate() {
                for (col, color) in sprite_row(attr, mapping, &ppu.vram, &ppu.palette, row) {
                    line[col] = color;
                }
            }

            let x = attr.x_coordinate() as i32;
            let y = attr.y_coordinate() as i32;
            let hidden = !attr.affine() && attr.disable_obj();
            let transform = if attr.affine() {
                format!(
                    "Affine #{}, double size {}",
                    attr.rotation_scaling_parameter(),
                    attr.double_size()
                )
            } else {
                format!(
                    "Flip H={} V={}",
                    attr.horizontal_flip(),
                    attr.vertical_flip()
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
                        "VRAM",
                        format!("{:08X}", VRAM_BASE as usize + attr.tile_addr(mapping, 0, 0)),
                    ),
                    Field::new("Colors", attr.color_depth().to_string()),
                    Field::new("Palette", attr.palette_number().to_string()),
                    Field::new("Priority", attr.priority().to_string()),
                    Field::new("Mode", mode.to_string()),
                    Field::new("Transform", transform),
                    Field::new("Mosaic", attr.mosaic().to_string()),
                    Field::new("Hidden", hidden.to_string()),
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
                        FlagView::new("BG0", control.enable_bg0()),
                        FlagView::new("BG1", control.enable_bg1()),
                        FlagView::new("BG2", control.enable_bg2()),
                        FlagView::new("BG3", control.enable_bg3()),
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

    for (id, title, kind, location) in [
        (
            panel::PPU,
            "PPU",
            PanelKind::Structured,
            DockLocation::Right,
        ),
        (
            panel::PALETTE,
            "Palettes",
            PanelKind::Palette,
            DockLocation::Left,
        ),
        (
            panel::BITMAP,
            "Background Layers",
            PanelKind::Bitmap,
            DockLocation::Center,
        ),
        (
            panel::SPRITES,
            "Sprites",
            PanelKind::Sprites,
            DockLocation::Center,
        ),
    ] {
        manifest.panels.push(
            PanelDescriptor::new(id, title, kind, location, RefreshPolicy::OnPause).tag(workspace),
        );
    }
}

impl VideoDebug for GbaDebug {
    fn resources(&self) -> Vec<VideoResource> {
        let palette = VideoResource::new(PALETTE, "Palette", VideoResourceKind::Palette);
        let sprites = VideoResource::new(SPRITES, "Objects", VideoResourceKind::Sprites);
        let layers = BG_LAYERS
            .iter()
            .map(|&(id, name)| VideoResource::new(id, name, VideoResourceKind::Bitmap));

        [palette, sprites].into_iter().chain(layers).collect()
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

    fn sprites(&self, resource: &VideoResourceId) -> Option<Sprites> {
        (resource.as_str() == SPRITES).then(|| sprites(&self.core))
    }

    fn bitmap(&self, resource: &VideoResourceId) -> Option<BitmapImage> {
        (bg_layer(resource.as_str())? == 2)
            .then(|| bitmap_bg2(&self.core))
            .flatten()
    }

    fn revision(&self, resource: &VideoResourceId) -> Option<VideoRevision> {
        let revisions = self.core.ppu.revisions;
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
                bg_layer(id)?;
                let mut hash = DefaultHasher::new();
                revisions.vram.hash(&mut hash);
                revisions.palette.hash(&mut hash);
                self.core.ppu.dispctrl.raw().hash(&mut hash);
                hash.finish()
            }
        };

        Some(VideoRevision {
            pixels,
            metadata: 0,
            overlay: 0,
        })
    }
}
