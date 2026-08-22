use crate::SnesBus;
use crate::mmio::device::{Access, IoRegister, RegEntry};
use crate::mmio::regs::{
    HvStatusJoypadBusy, MemorySelect, NmiTimerEnable, TimerUp, VblankFlagCpuVersion,
};
use crate::mmio::{MemoryMap, RDIO, WRIO};
use crate::ppu::regs::{HTIMEH, HTIMEL, VTIMEH, VTIMEL};

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![
        NmiTimerEnable::entry(),
        Wrio::entry(),
        HTimeLow::entry(),
        HTimeHigh::entry(),
        VTimeLow::entry(),
        VTimeHigh::entry(),
        MemorySelect::entry(),
        VblankFlagCpuVersion::entry(),
        TimerUp::entry(),
        HvStatusJoypadBusy::entry(),
        Rdio::entry(),
    ]
}

struct Wrio;

impl IoRegister for Wrio {
    const START: u16 = WRIO;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        let latch_pin_was_high = snes.mmu.latch_pin_high();
        snes.mmu.wrio = value;

        if latch_pin_was_high && !snes.mmu.latch_pin_high() {
            crate::ppu::latch_hv_counters(snes);
        }
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.mmu.wrio
    }
}

struct Rdio;

impl IoRegister for Rdio {
    const START: u16 = RDIO;

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.mmu.wrio
    }
}

struct HTimeLow;

impl IoRegister for HTimeLow {
    const START: u16 = HTIMEL;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.irq.h_timer.set_target_low(value);
        crate::interrupt::hv_targets_updated(snes);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.irq.h_timer.target_low()
    }
}

struct HTimeHigh;

impl IoRegister for HTimeHigh {
    const START: u16 = HTIMEH;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.irq.h_timer.set_target_high(value);
        crate::interrupt::hv_targets_updated(snes);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.irq.h_timer.target_high()
    }
}

struct VTimeLow;

impl IoRegister for VTimeLow {
    const START: u16 = VTIMEL;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.irq.v_timer.set_target_low(value);
        crate::interrupt::hv_targets_updated(snes);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.irq.v_timer.target_low()
    }
}

struct VTimeHigh;

impl IoRegister for VTimeHigh {
    const START: u16 = VTIMEH;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.irq.v_timer.set_target_high(value);
        crate::interrupt::hv_targets_updated(snes);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.irq.v_timer.target_high()
    }
}
