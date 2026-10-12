pub(crate) mod affine;
pub(crate) mod blend;
pub(crate) mod io;
pub(crate) mod mosaic;
pub(crate) mod regs;
pub(crate) mod renderer;
pub(crate) mod sprite;
pub(crate) mod tile;
pub(crate) mod window;

use crate::bus::Width;
#[cfg(feature = "debug")]
use crate::debug::window_trace::{WindowDebugRow, WindowTrace};
use crate::ppu::regs::{BackgroundKind, BackgroundMode};
use crate::ppu::sprite::OBJ_TILE_OFFSET;
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use affine::AffineMatrix;
use blend::BlendView;
use mosaic::MosaicSize;
use regs::{
    AffineParameter, BackgroundControl, BackgroundOffset, BlendAlpha, BlendBrightness,
    BlendControl, DisplayControl, DisplayStatus, Mosaic, ReferencePoint, VerticalCount,
    WindowControl, WindowXOffset, WindowYOffset,
};
use window::WindowView;

const SCANLINES_PER_FRAME: u8 = 228;
const VBLANK_START_SCANLINE: u8 = 160;
const VBLANK_END_SCANLINE: u8 = 227;

pub(crate) const PALETTE_BASE: u32 = 0x0500_0000;
pub(crate) const PALETTE_END: u32 = 0x05FF_FFFF;
pub(crate) const PALETTE_SIZE: u32 = 0x400;
pub(crate) const VRAM_BASE: u32 = 0x0600_0000;
pub(crate) const VRAM_END: u32 = 0x06FF_FFFF;
pub(crate) const VRAM_SIZE: u32 = 0x18000;
pub(crate) const OAM_BASE: u32 = 0x0700_0000;
pub(crate) const OAM_END: u32 = 0x07FF_FFFF;
pub(crate) const OAM_SIZE: u32 = 0x400;

pub(crate) const SCANLINE_CYCLES: u64 = 1232;
pub(crate) const HBLANK_START_CYCLES: u64 = 1006;
pub(crate) const VBLANK_START_CYCLES: u64 = VBLANK_START_SCANLINE as u64 * SCANLINE_CYCLES;
pub(crate) const VBLANK_END_CYCLES: u64 = VBLANK_END_SCANLINE as u64 * SCANLINE_CYCLES;
pub(crate) const FRAME_CYCLES: u64 = SCANLINES_PER_FRAME as u64 * SCANLINE_CYCLES;

pub(crate) type Rgba = [u8; 4];

pub(crate) fn rgba(color: u16) -> Rgba {
    let expand = |shift: u32| {
        let channel = ((color >> shift) & 0x1F) as u8;
        (channel << 3) | (channel >> 2)
    };

    [expand(0), expand(5), expand(10), 255]
}

pub struct Ppu {
    #[cfg(feature = "debug")]
    pub(crate) revisions: Revisions,
    #[cfg(feature = "debug")]
    pub(crate) window_trace: WindowTrace,

    pub(crate) dispstat: DisplayStatus,
    pub(crate) dispctrl: DisplayControl,
    pub(crate) vcount: VerticalCount,
    pub(crate) bgcnt: [BackgroundControl; 4],
    pub(crate) bghofs: [BackgroundOffset; 4],
    pub(crate) bgvofs: [BackgroundOffset; 4],
    /// PA, PB, PC, PD for BG2 and BG3.
    pub(crate) bg_matrix: [[AffineParameter; 4]; 2],
    /// X, Y for BG2 and BG3.
    pub(crate) bg_ref: [[ReferencePoint; 2]; 2],
    /// Internal reference points: latched on write and at vblank, advanced per scanline.
    pub(crate) bg_ref_internal: [[i32; 2]; 2],
    pub(crate) winh: [WindowXOffset; 2],
    pub(crate) winv: [WindowYOffset; 2],
    pub(crate) winin: WindowControl,
    pub(crate) winout: WindowControl,

    pub(crate) mosaic: Mosaic,
    pub(crate) bldcnt: BlendControl,
    pub(crate) bldalpha: BlendAlpha,
    pub(crate) bldy: BlendBrightness,

    pub(crate) vram: Vec<u8>,
    pub(crate) palette: Vec<u8>,
    pub(crate) oam: Vec<u8>,

    pub(crate) frame: Vec<Rgba>,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "debug")]
            window_trace: WindowTrace::default(),
            #[cfg(feature = "debug")]
            revisions: Revisions {
                palette: 0,
                vram: 0,
                oam: 0,
            },
            dispstat: DisplayStatus::zeroed().with_vcounter(true),
            dispctrl: DisplayControl::zeroed(),
            vcount: VerticalCount::zeroed(),
            bgcnt: [BackgroundControl::zeroed(); 4],
            bghofs: [BackgroundOffset::zeroed(); 4],
            bgvofs: [BackgroundOffset::zeroed(); 4],
            bg_matrix: [[
                AffineParameter::from_raw(0x100),
                AffineParameter::zeroed(),
                AffineParameter::zeroed(),
                AffineParameter::from_raw(0x100),
            ]; 2],
            bg_ref: [[ReferencePoint::zeroed(); 2]; 2],
            bg_ref_internal: [[0; 2]; 2],
            winh: [WindowXOffset::zeroed(); 2],
            winv: [WindowYOffset::zeroed(); 2],
            winin: WindowControl::zeroed(),
            winout: WindowControl::zeroed(),
            mosaic: Mosaic::zeroed(),
            bldcnt: BlendControl::zeroed(),
            bldalpha: BlendAlpha::zeroed(),
            bldy: BlendBrightness::zeroed(),
            vram: vec![0; VRAM_SIZE as usize],
            palette: vec![0; PALETTE_SIZE as usize],
            oam: vec![0; OAM_SIZE as usize],
            frame: vec![[255; 4]; (SCREEN_WIDTH * SCREEN_HEIGHT) as usize],
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self {
            #[cfg(feature = "debug")]
            revisions: {
                let mut revisions = self.revisions;
                revisions.bump();
                revisions
            },
            ..Self::new()
        };
    }

    pub(crate) fn render_scanline(&mut self) {
        let line = self.vcount.scanline() as usize;
        let memory = self.memory_view();

        let mut bg_lines = [[None; SCREEN_WIDTH as usize]; 4];
        let mut bg_order = Vec::with_capacity(4);

        for (index, bg_line) in bg_lines.iter_mut().enumerate() {
            if !self.dispctrl.bg_enabled(index) {
                continue;
            }

            let background = self.background_view(index);
            match background.kind {
                BackgroundKind::Text => {
                    renderer::render_background_line(background, memory, line, bg_line)
                }
                BackgroundKind::Bitmap => {
                    if let Some(layout) = self.bitmap_layout() {
                        let mosaic = background.mosaic;
                        renderer::render_bitmap_line(layout, mosaic, memory, line, bg_line);
                    }
                }
                BackgroundKind::Affine => {
                    let affine = index - 2;
                    let matrix = AffineMatrix::from_params(self.bg_matrix[affine]);
                    let [x, y] = self.bg_ref_internal[affine];
                    let origin = (x, y);
                    renderer::render_affine_background_line(
                        background, matrix, origin, memory, bg_line,
                    );
                }
                BackgroundKind::Unavailable => continue,
            }
            bg_order.push((index, self.bgcnt[index].priority()));
        }

        bg_order.sort_by_key(|&(_, priority)| priority);

        let mut sprite_line = [None; SCREEN_WIDTH as usize];
        let mut obj_window = [false; SCREEN_WIDTH as usize];
        renderer::render_sprite_line(
            self.dispctrl,
            self.mosaic.obj_size(),
            memory,
            line,
            &mut sprite_line,
            &mut obj_window,
        );

        let backdrop = palette_color(memory.palette, 0);
        let windows = self.window_view();
        let blending = self.blend_view();

        #[cfg(feature = "debug")]
        self.window_trace
            .record(line, WindowDebugRow::capture(self, obj_window));

        let start = line * SCREEN_WIDTH as usize;
        let output = (&mut self.frame[start..start + SCREEN_WIDTH as usize])
            .try_into()
            .unwrap();
        renderer::composite(
            &bg_order,
            &bg_lines,
            &sprite_line,
            &obj_window,
            backdrop,
            windows,
            blending,
            line,
            output,
        );
    }

    fn background_view(&self, index: usize) -> BackgroundView {
        let ctrl = self.bgcnt[index];
        BackgroundView {
            kind: self.dispctrl.mode().background_kind(index),
            ctrl,
            hofs: self.bghofs[index].offset(),
            vofs: self.bgvofs[index].offset(),
            mosaic: if ctrl.mosaic() {
                self.mosaic.bg_size()
            } else {
                MosaicSize::NONE
            },
        }
    }

    pub(crate) fn window_view(&self) -> WindowView {
        WindowView {
            enabled: [
                self.dispctrl.enable_window0(),
                self.dispctrl.enable_window1(),
            ],
            obj_enabled: self.dispctrl.enable_obj_window(),
            winh: self.winh,
            winv: self.winv,
            winin: self.winin,
            winout: self.winout,
        }
    }

    fn blend_view(&self) -> BlendView {
        let coefficient = |value: u8| u16::from(value.min(16));
        BlendView {
            control: self.bldcnt,
            eva: coefficient(self.bldalpha.eva()),
            evb: coefficient(self.bldalpha.evb()),
            evy: coefficient(self.bldy.ev()),
        }
    }

    pub(crate) fn save_io(&self, io: &mut [u8]) {
        io[0x4C..0x4E].copy_from_slice(&self.mosaic.raw().to_le_bytes());
        io[0x50..0x52].copy_from_slice(&self.bldcnt.raw().to_le_bytes());
        io[0x52..0x54].copy_from_slice(&self.bldalpha.raw().to_le_bytes());
        io[0x54..0x56].copy_from_slice(&self.bldy.raw().to_le_bytes());
    }

    pub(crate) fn load_io(&mut self, io: &[u8]) {
        let half = |offset: usize| u16::from_le_bytes(io[offset..(offset + 2)].try_into().unwrap());
        self.mosaic = Mosaic::from_raw(half(0x4C));
        self.bldcnt = BlendControl::from_raw(half(0x50) & BlendControl::WRITE_MASK);
        self.bldalpha = BlendAlpha::from_raw(half(0x52) & BlendAlpha::WRITE_MASK);
        self.bldy = BlendBrightness::from_raw(half(0x54) & BlendBrightness::WRITE_MASK);
    }

    pub(crate) fn memory_view(&self) -> VideoMemoryView<'_> {
        VideoMemoryView {
            vram: &self.vram,
            palette: &self.palette,
            oam: &self.oam,
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn palette_color(&self, index: usize) -> u16 {
        palette_color(&self.palette, index)
    }

    pub(crate) fn start_hblank(&mut self) {
        self.dispstat.set_hblank(true);

        if self.vcount.scanline() < SCREEN_HEIGHT as u8 {
            self.render_scanline();
            self.step_reference_points();
        }
    }

    pub(crate) fn start_vblank(&mut self) {
        #[cfg(feature = "debug")]
        self.window_trace.finish_frame();
        self.dispstat.set_vblank(true);
        self.reload_reference_points();
    }

    /// Advances the internal reference points by PB/PD to the start of the next scanline.
    fn step_reference_points(&mut self) {
        for i in 0..2 {
            let [_, pb, _, pd] = self.bg_matrix[i];
            self.bg_ref_internal[i][0] += pb.value();
            self.bg_ref_internal[i][1] += pd.value();
        }
    }

    fn reload_reference_points(&mut self) {
        self.bg_ref_internal = self.bg_ref.map(|point| point.map(ReferencePoint::value));
    }

    pub(crate) fn end_vblank(&mut self) {
        self.dispstat.set_vblank(false);
    }

    pub(crate) fn start_scanline(&mut self) {
        let line = (self.vcount.scanline() + 1) % SCANLINES_PER_FRAME;
        self.vcount.set_scanline(line);

        self.dispstat.set_hblank(false);
        self.dispstat
            .set_vcounter(self.dispstat.vcount_compare() == line);
    }

    pub(crate) fn bitmap_layout(&self) -> Option<BitmapLayout> {
        let page = self.dispctrl.frame_select().vram_offset();
        let (width, height, bytes_per_pixel, base) = match self.dispctrl.mode() {
            BackgroundMode::Mode3 => (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize, 2, 0),
            BackgroundMode::Mode4 => (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize, 1, page),
            BackgroundMode::Mode5 => (160, 128, 2, page),
            _ => return None,
        };

        Some(BitmapLayout {
            width,
            height,
            bytes_per_pixel,
            base,
        })
    }

    fn read_palette(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.palette, address, width, |address| {
            address as usize & (PALETTE_SIZE as usize - 1)
        })
    }

    fn write_palette(&mut self, mut address: u32, width: Width, value: u32) {
        if width == Width::Byte {
            address &= !1;
            self::write_memory(&mut self.palette, address + 1, width, value, |address| {
                address as usize & (PALETTE_SIZE as usize - 1)
            });
        }

        #[cfg(feature = "debug")]
        {
            self.revisions.palette += 1;
        }

        self::write_memory(&mut self.palette, address, width, value, |address| {
            address as usize & (PALETTE_SIZE as usize - 1)
        });
    }

    fn read_vram(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.vram, address, width, self::vram_offset)
    }

    fn write_vram(&mut self, mut address: u32, width: Width, value: u32) {
        if width == Width::Byte {
            if self::vram_offset(address) >= self.obj_offset() {
                return;
            }

            address &= !1;
            self::write_memory(&mut self.vram, address + 1, width, value, self::vram_offset);
        }

        #[cfg(feature = "debug")]
        {
            self.revisions.vram += 1;
        }

        self::write_memory(&mut self.vram, address, width, value, self::vram_offset);
    }

    fn read_oam(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.oam, address, width, |address| {
            address as usize & (OAM_SIZE as usize - 1)
        })
    }

    fn write_oam(&mut self, address: u32, width: Width, value: u32) {
        if width == Width::Byte {
            tracing::error!("Attempted to write to OAM with byte width");
            return;
        }

        #[cfg(feature = "debug")]
        {
            self.revisions.oam += 1;
        }

        self::write_memory(&mut self.oam, address, width, value, |address| {
            address as usize & (OAM_SIZE as usize - 1)
        });
    }

    fn obj_offset(&self) -> usize {
        match self.dispctrl.mode() {
            BackgroundMode::Mode0 | BackgroundMode::Mode1 | BackgroundMode::Mode2 => {
                OBJ_TILE_OFFSET
            }
            _ => 0x14000,
        }
    }

    pub fn frame(&self) -> &[Rgba] {
        &self.frame
    }
}

fn read_memory(memory: &[u8], address: u32, width: Width, offset: impl Fn(u32) -> usize) -> u32 {
    let mut value = 0;
    for lane in 0..width.bytes() {
        value |= u32::from(memory[offset(address.wrapping_add(lane))]) << (lane * 8);
    }
    value
}

fn write_memory(
    memory: &mut [u8],
    address: u32,
    width: Width,
    value: u32,
    offset: impl Fn(u32) -> usize,
) {
    for lane in 0..width.bytes() {
        memory[offset(address.wrapping_add(lane))] = (value >> (lane * 8)) as u8;
    }
}

fn palette_color(palette: &[u8], index: usize) -> u16 {
    let offset = (index * 2) & (PALETTE_SIZE as usize - 1);
    u16::from_le_bytes([palette[offset], palette[offset + 1]])
}

pub(crate) fn vram_offset(address: u32) -> usize {
    // Each 128 KiB window repeats the last 32 KiB of the 96 KiB VRAM.
    let offset = address as usize & 0x1FFFF;
    if offset >= VRAM_SIZE as usize {
        offset - 0x8000
    } else {
        offset
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BitmapLayout {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) bytes_per_pixel: usize,
    pub(crate) base: usize,
}

#[cfg(feature = "debug")]
impl BitmapLayout {
    pub(crate) fn address(self) -> u32 {
        VRAM_BASE + self.base as u32
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BackgroundView {
    pub(crate) ctrl: BackgroundControl,
    pub(crate) kind: BackgroundKind,
    pub(crate) hofs: u16,
    pub(crate) vofs: u16,
    pub(crate) mosaic: MosaicSize,
}

#[derive(Clone, Copy)]
pub(crate) struct VideoMemoryView<'a> {
    pub(crate) vram: &'a [u8],
    pub(crate) palette: &'a [u8],
    pub(crate) oam: &'a [u8],
}

#[cfg(feature = "debug")]
#[derive(Clone, Copy)]
pub(crate) struct Revisions {
    pub(crate) palette: u64,
    pub(crate) vram: u64,
    pub(crate) oam: u64,
}

#[cfg(feature = "debug")]
impl Revisions {
    pub(crate) fn bump(&mut self) {
        self.palette += 1;
        self.vram += 1;
        self.oam += 1;
    }
}
