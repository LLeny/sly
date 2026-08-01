pub mod io;
pub mod map;
pub mod regs;

use crate::Snes;
use crate::mmio::MemoryMap;
use crate::scheduler::EventKind;
use regs::{Cfgr, Clsr, Scmr, Sfr};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

pub const VERSION_CODE: u8 = 0x04;

#[derive(Default, Serialize, Deserialize)]
pub struct SuperFx {
    pub r: [u16; 16],
    pub sfr: Sfr,
    pub pbr: u8,
    pub rombr: u8,
    pub rambr: u8,
    pub cbr: u16,
    pub scbr: u8,
    pub scmr: Scmr,
    pub cfgr: Cfgr,
    pub clsr: Clsr,
    pub bramr: u8,
    latch: u8,
    pub cache: CodeCache,
    tick_scheduled: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CodeCache {
    #[serde(with = "BigArray")]
    pub data: [u8; 0x200],
    pub valid: u32,
}

impl Default for CodeCache {
    fn default() -> Self {
        Self {
            data: [0; 0x200],
            valid: 0,
        }
    }
}

impl CodeCache {
    pub fn invalidate(&mut self) {
        self.valid = 0;
    }

    pub fn write(&mut self, index: usize, value: u8) {
        self.data[index] = value;
        if index & 0xF == 0xF {
            self.valid |= 1 << (index >> 4);
        }
    }
}

pub(crate) fn start<M: MemoryMap>(snes: &mut Snes<M>) {
    snes.superfx.sfr.set_go(true);

    if !snes.superfx.tick_scheduled {
        snes.superfx.tick_scheduled = true;
        let now = snes.scheduler.cycles();
        snes.scheduler.schedule_event(now + 1, EventKind::GsuTick);
    }
}

pub(crate) fn stop<M: MemoryMap>(snes: &mut Snes<M>) {
    snes.superfx.sfr.set_go(false);
    snes.superfx.sfr.set_irq(true);

    if !snes.superfx.cfgr.irq_masked() {
        snes.irq.set_cart_irq(true);
    }
}

pub(crate) fn gsu_tick<M: MemoryMap>(snes: &mut Snes<M>, _deadline: u64) {
    snes.superfx.tick_scheduled = false;
    if !snes.superfx.sfr.go() {
        return;
    }

    // TODO: STUBBED OUT FOR NOW
    self::stop(snes);
}
