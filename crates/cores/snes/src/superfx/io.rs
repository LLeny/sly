use crate::Bus;
use crate::mmio::MemoryMap;
use crate::mmio::device::{IoRegister, RegEntry};
use crate::superfx::regs::{BRAMR, CBR, Cfgr, Clsr, PBR, RAMBR, ROMBR, SCBR, Scmr, Sfr, VCR};

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![Gsu::entry()]
}

const SFR_LOW: u16 = Sfr::ADDRESS;
const SFR_HIGH: u16 = Sfr::ADDRESS + 1;
const CBR_LOW: u16 = CBR;
const CBR_HIGH: u16 = CBR + 1;

enum Target {
    GprLow(usize),
    GprHigh(usize),
    Status(u16),
    Cache(usize),
}

fn map(reg: u16) -> Target {
    if let 0x3100..=0x32FF = reg {
        return Target::Cache((reg - 0x3100) as usize);
    }

    match reg & 0x3F {
        low @ 0x00..=0x1F => {
            let n = (low >> 1) as usize;
            if low & 1 == 0 {
                Target::GprLow(n)
            } else {
                Target::GprHigh(n)
            }
        }
        low => Target::Status(0x3030 | (low & 0x0F)),
    }
}

pub(crate) struct Gsu;

impl IoRegister for Gsu {
    const START: u16 = 0x3000;
    const END: u16 = 0x34FF;

    fn read<M: MemoryMap>(snes: &mut Bus<M>, reg: u16) -> u8 {
        let target = map(reg);
        let value = peek_target(snes, &target);

        if let Target::Status(SFR_HIGH) = target {
            snes.superfx.sfr.set_irq(false);
            snes.irq.set_cart_irq(false);
        }

        value
    }

    fn write<M: MemoryMap>(snes: &mut Bus<M>, reg: u16, value: u8) {
        match map(reg) {
            Target::GprLow(_) => snes.superfx.latch = value,
            Target::GprHigh(n) => {
                snes.superfx.r[n] = snes.superfx.latch as u16 | ((value as u16) << 8);
                if n == 14 {
                    snes.superfx.trigger_rom_prefetch();
                    if !snes.superfx.sfr.go() {
                        crate::superfx::interpreter::drain_buffers(snes);
                    }
                } else if n == 15 {
                    crate::superfx::start(snes);
                }
            }
            Target::Cache(index) => {
                let fx = &mut snes.superfx;
                fx.cache.write(fx.cbr, index, value);
            }
            Target::Status(status) => match status {
                SFR_LOW => {
                    let sfr = &mut snes.superfx.sfr;
                    let bits = Sfr::from_raw(value as u16);
                    sfr.set_zero(bits.zero());
                    sfr.set_carry(bits.carry());
                    sfr.set_sign(bits.sign());
                    sfr.set_overflow(bits.overflow());

                    if bits.go() {
                        crate::superfx::start(snes);
                    } else {
                        snes.superfx.force_stop();
                    }
                }
                BRAMR => snes.superfx.bramr = value & 1,
                PBR => {
                    snes.superfx.pbr = value & 0x7F;
                    snes.superfx.cache.invalidate();
                }
                Cfgr::ADDRESS => snes.superfx.cfgr = Cfgr::from_raw(value),
                SCBR => snes.superfx.scbr = value,
                Clsr::ADDRESS => snes.superfx.clsr = Clsr::from_raw(value),
                Scmr::ADDRESS => {
                    snes.superfx.scmr = Scmr::from_raw(value);
                    crate::superfx::bus_grants_updated(snes);
                }
                _ => {}
            },
        }
    }

    fn peek<M: MemoryMap>(snes: &Bus<M>, reg: u16) -> u8 {
        peek_target(snes, &map(reg))
    }
}

fn peek_target<M: MemoryMap>(snes: &Bus<M>, target: &Target) -> u8 {
    match target {
        Target::GprLow(n) => snes.superfx.r[*n] as u8,
        Target::GprHigh(n) => (snes.superfx.r[*n] >> 8) as u8,
        Target::Cache(index) => snes.superfx.cache.read(snes.superfx.cbr, *index),
        Target::Status(status) => match *status {
            SFR_LOW => snes.superfx.sfr.raw() as u8,
            SFR_HIGH => (snes.superfx.sfr.raw() >> 8) as u8,
            PBR => snes.superfx.pbr,
            ROMBR => snes.superfx.rombr,
            VCR => crate::superfx::VERSION_CODE,
            RAMBR => snes.superfx.rambr,
            CBR_LOW => snes.superfx.cbr as u8,
            CBR_HIGH => (snes.superfx.cbr >> 8) as u8,
            _ => 0x00,
        },
    }
}
