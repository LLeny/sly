use crate::Snes;
use crate::mmio::MemoryMap;
use crate::mmio::regs::{NmiTimerEnable, VblankFlagCpuVersion};
use serde::{Deserialize, Serialize};

const MASTER_CLOCKS_PER_DOT: u64 = 4;
const MAX_H_COUNTER: u64 = 339;
const H_COUNTER_OFFSET: u64 = 10;
const H_TIMER_IRQ_OFFSET: u64 = H_COUNTER_OFFSET + MASTER_CLOCKS_PER_DOT;

#[derive(Default, Serialize, Deserialize)]
pub struct EdgeLatch {
    level: bool,
    pending: bool,
}

impl EdgeLatch {
    pub fn set_level(&mut self, level: bool) {
        if level && !self.level {
            self.pending = true;
        }

        self.level = level;
    }

    pub fn take(&mut self) -> bool {
        core::mem::take(&mut self.pending)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Timer {
    pub(crate) target: u16,
}

impl Default for Timer {
    fn default() -> Self {
        Self { target: 0x1FF }
    }
}

impl Timer {
    pub(crate) fn set_target_low(&mut self, value: u8) {
        self.target = (self.target & 0x100) | value as u16;
    }

    pub(crate) fn set_target_high(&mut self, value: u8) {
        self.target = (self.target & 0x0FF) | ((value as u16 & 1) << 8);
    }

    pub(crate) fn target_low(&self) -> u8 {
        self.target as u8
    }

    pub(crate) fn target_high(&self) -> u8 {
        (self.target >> 8) as u8
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Irq {
    nmitimen: NmiTimerEnable,
    rdnmi: VblankFlagCpuVersion,
    nmi: EdgeLatch,
    timer_flag: bool,
    cart_irq: bool, // Set when GSU is stopped
    pub(crate) h_timer: Timer,
    pub(crate) v_timer: Timer,
}

impl Irq {
    fn sync(&mut self) {
        self.nmi
            .set_level(self.rdnmi.vblank() && self.nmitimen.vblank_nmi_enable());
    }

    pub fn set_vblank(&mut self, active: bool) {
        self.rdnmi.set_vblank(active);
        self.sync();
    }

    pub fn write_nmitimen(&mut self, value: u8) {
        self.nmitimen = NmiTimerEnable::from_raw(value);
        if self.nmitimen.hv_timer_condition() == 0 {
            self.timer_flag = false;
        }
        self.sync();
    }

    pub fn nmitimen_raw(&self) -> u8 {
        self.nmitimen.raw()
    }

    pub fn joypad_autoread_enabled(&self) -> bool {
        self.nmitimen.joypad_autoread_enable()
    }

    pub fn hv_timer_condition(&self) -> u8 {
        self.nmitimen.hv_timer_condition()
    }

    pub fn raise_timer(&mut self) {
        self.timer_flag = true;
    }

    pub fn set_cart_irq(&mut self, active: bool) {
        self.cart_irq = active;
    }

    pub fn irq_pending(&self) -> bool {
        self.timer_flag || self.cart_irq
    }

    pub fn read_timeup(&mut self) -> u8 {
        let value = self.peek_timeup();
        self.timer_flag = false;
        value
    }

    pub fn peek_timeup(&self) -> u8 {
        (self.timer_flag as u8) << 7
    }

    pub fn read_rdnmi(&mut self) -> u8 {
        let value = self.rdnmi.raw();
        self.set_vblank(false);
        value
    }

    pub fn rdnmi_raw(&self) -> u8 {
        self.rdnmi.raw()
    }

    pub fn take_nmi(&mut self) -> bool {
        self.nmi.take()
    }
}

pub(crate) fn hv_timer_line<M: MemoryMap>(snes: &mut Snes<M>, deadline: u64) {
    let condition = snes.irq.hv_timer_condition();
    if condition == 0 {
        return;
    }

    let scanline = snes.scheduler.scanline_at(deadline);
    let htime = snes.irq.h_timer.target as u64;
    let vtime = snes.irq.v_timer.target as u64;

    match condition {
        // H-timer: fire every scanline when H reaches HTIME
        1 => schedule_h_fire(snes, deadline, htime),
        // V-timer: fire once per frame at V == VTIME, H = 0
        2 if scanline == vtime => snes.irq.raise_timer(),
        // H+V-timer: fire at V == VTIME when H reaches HTIME
        3 if scanline == vtime => schedule_h_fire(snes, deadline, htime),
        _ => {}
    }
}

fn schedule_h_fire<M: MemoryMap>(snes: &mut Snes<M>, line_start: u64, htime: u64) {
    if let Some(deadline) = h_timer_irq_deadline(line_start, htime) {
        snes.scheduler
            .schedule_event(deadline, crate::scheduler::EventKind::HvTimerFire);
    }
}

fn h_timer_irq_deadline(line_start: u64, htime: u64) -> Option<u64> {
    (htime <= MAX_H_COUNTER)
        .then(|| line_start + htime * MASTER_CLOCKS_PER_DOT + H_TIMER_IRQ_OFFSET)
}

pub(crate) fn hv_timer_fire<M: MemoryMap>(snes: &mut Snes<M>, _deadline: u64) {
    if snes.irq.hv_timer_condition() & 1 != 0 {
        snes.irq.raise_timer();
    }
}
