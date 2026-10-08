use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use debugger_model::{
    BitmapImage, BitmapSource, Field, FlagView, MemoryAddress, PaletteColor, PaletteView, Row,
    Section, StructuredView, VideoDebug, VideoResource, VideoResourceId, VideoResourceKind,
    VideoRevision, panel,
};
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

use super::GbaDebug;
use crate::Gba;
use crate::ppu::regs::BackgroundMode;
use crate::ppu::{bitmap_layout, render_bg_scanline, rgba};

const PALETTE: &str = "gba.palette";

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
    let (width, height, bytes_per_pixel, base) = bitmap_layout(control)?;
    let mut pixels = vec![[0; 4]; width * height];
    for (line, output) in pixels.chunks_exact_mut(width).enumerate() {
        render_bg_scanline(control, &core.ppu.vram, &core.ppu.palette, line, output);
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
                addr: 0x0600_0000 + base as u32,
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

pub(super) fn ppu_view(core: &Gba) -> StructuredView {
    let control = core.ppu.dispctrl;
    let status = core.ppu.dispstat;
    let vcount = core.ppu.vcount;
    let field = |label: &str, value: String| Row::Field(Field::new(label, value));

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
    ] {
        manifest.panels.push(
            PanelDescriptor::new(id, title, kind, location, RefreshPolicy::OnPause).tag(workspace),
        );
    }
}

impl VideoDebug for GbaDebug {
    fn resources(&self) -> Vec<VideoResource> {
        let palette = VideoResource::new(PALETTE, "Palette", VideoResourceKind::Palette);
        let layers = BG_LAYERS
            .iter()
            .map(|&(id, name)| VideoResource::new(id, name, VideoResourceKind::Bitmap));

        std::iter::once(palette).chain(layers).collect()
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

    fn bitmap(&self, resource: &VideoResourceId) -> Option<BitmapImage> {
        (bg_layer(resource.as_str())? == 2)
            .then(|| bitmap_bg2(&self.core))
            .flatten()
    }

    fn revision(&self, resource: &VideoResourceId) -> Option<VideoRevision> {
        let revisions = self.core.ppu.revisions;
        let pixels = match resource.as_str() {
            PALETTE => revisions.palette,
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
