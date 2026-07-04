use crate::mmio::regs::{NmiTimerEnable, VblankFlagCpuVersion};

#[derive(Default)]
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

#[derive(Default)]
pub struct Irq {
    nmitimen: NmiTimerEnable,
    rdnmi: VblankFlagCpuVersion,
    nmi: EdgeLatch,
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
        self.sync();
    }

    pub fn nmitimen_raw(&self) -> u8 {
        self.nmitimen.raw()
    }

    pub fn joypad_autoread_enabled(&self) -> bool {
        self.nmitimen.joypad_autoread_enable()
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
