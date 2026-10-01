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
use crate::ppu::regs::DisplayFrame;
use crate::ppu::rgba;
use crate::{Gba, SCREEN_HEIGHT, SCREEN_WIDTH};

const PALETTE: &str = "gba.palette";

#[derive(Clone, Copy)]
enum BgLayer {
    Stub,
    Mode4(DisplayFrame),
}

const BG_LAYERS: [(&str, &str, Option<&str>, BgLayer); 3] = [
    (
        "gba.bg.mode4.frame0",
        "Mode 4",
        Some("Frame 0"),
        BgLayer::Mode4(DisplayFrame::Frame0),
    ),
    (
        "gba.bg.mode4.frame1",
        "Mode 4",
        Some("Frame 1"),
        BgLayer::Mode4(DisplayFrame::Frame1),
    ),
    ("gba.bg.mode5", "Mode 5 (stub)", None, BgLayer::Stub),
];

fn bg_layer(id: &str) -> Option<BgLayer> {
    BG_LAYERS
        .iter()
        .find(|&&(candidate, ..)| candidate == id)
        .map(|&(.., layer)| layer)
}

impl VideoDebug for GbaDebug {
    fn resources(&self) -> Vec<VideoResource> {
        let palette = VideoResource::new(PALETTE, "Palette", VideoResourceKind::Palette);
        let layers = BG_LAYERS.iter().map(|&(id, name, variant, _)| {
            let resource = VideoResource::new(id, name, VideoResourceKind::Bitmap);
            match variant {
                Some(variant) => resource.variant(variant),
                None => resource,
            }
        });

        std::iter::once(palette).chain(layers).collect()
    }

    fn revision(&self, resource: &VideoResourceId) -> Option<VideoRevision> {
        let revisions = self.core.ppu.revisions;
        let pixels = match resource.as_str() {
            PALETTE => revisions.palette,
            id => match bg_layer(id)? {
                BgLayer::Stub => 0,
                BgLayer::Mode4(_) => revisions.vram + revisions.palette,
            },
        };

        Some(VideoRevision {
            pixels,
            metadata: 0,
            overlay: 0,
        })
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
        match bg_layer(resource.as_str())? {
            BgLayer::Stub => None,
            BgLayer::Mode4(frame) => Some(mode4(&self.core, frame)),
        }
    }
}

fn mode4(core: &Gba, frame: DisplayFrame) -> BitmapImage {
    let base = frame.vram_offset();
    let bitmap = &core.ppu.vram[base..base + core.ppu.frame.len()];

    BitmapImage {
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
        pixels: bitmap
            .iter()
            .map(|&index| rgba(core.ppu.palette_color(index.into())))
            .collect(),
        source: Some(BitmapSource {
            address: MemoryAddress {
                region: "gba.vram".into(),
                addr: 0x0600_0000 + base as u32,
            },
            bytes_per_pixel: 1,
        }),
        indices: Some(bitmap.iter().map(|&index| u16::from(index)).collect()),
        ..Default::default()
    }
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
