use std::collections::HashMap;

use debugger_model::{
    AccessSource, AccessTracking, AccessWidth, BreakpointKind, BreakpointMark, CpuId, Highlight,
    HookHit,
};

use super::{CPU, REGIONS, SPACE};
use crate::bus::CpuBus;
use crate::cpu::Bus;
use crate::ppu;

#[derive(Clone, Copy)]
struct Access {
    addr: u32,
    value: u32,
    width: AccessWidth,
    kind: BreakpointKind,
}

#[derive(Default)]
pub(super) struct Capture {
    tracking: AccessTracking,
    pub watch: bool,
    pub latest: Option<u32>,
    pc: u32,
    accesses: Vec<Access>,
    highlights: HashMap<u32, Highlight>,
}

impl Capture {
    pub fn clear(&mut self) {
        self.latest = None;
        self.accesses.clear();
        self.highlights.clear();
        self.watch = false;
    }

    pub fn set_tracking(&mut self, tracking: AccessTracking) {
        if self.tracking != tracking {
            self.clear();
        }
        self.tracking = tracking;
    }

    pub fn begin_instruction(&mut self, pc: u32) {
        self.pc = pc;
        self.accesses.clear();
    }

    fn record(&mut self, access: Access) {
        if self.tracking == AccessTracking::Off && !self.watch {
            return;
        }

        if self.tracking != AccessTracking::Off {
            self.latest = Some(access.addr);
        }

        if self.tracking == AccessTracking::Highlights {
            if self.highlights.len() >= 0x10000 {
                self.highlights.clear();
            }

            let highlight = match access.kind {
                BreakpointKind::Write => Highlight::Written,
                _ => Highlight::Read,
            };
            for lane in 0..access.width.bytes() {
                self.highlights
                    .insert(canonical(access.addr.wrapping_add(lane)), highlight);
            }
        }

        if self.watch {
            self.accesses.push(access);
        }
    }

    pub fn highlight(&self, addr: u32) -> Highlight {
        self.highlights
            .get(&canonical(addr))
            .copied()
            .unwrap_or_default()
    }

    pub fn matching_hits(&self, marks: &[BreakpointMark]) -> Vec<HookHit> {
        if self.accesses.is_empty() {
            return Vec::new();
        }

        let cpu = CpuId::from(CPU);
        let mut hits = Vec::new();
        for access in &self.accesses {
            let addr = canonical(access.addr);
            let region = REGIONS
                .iter()
                .skip(1)
                .find(|&&(_, _, base, size)| (base..base + size).contains(&addr))
                .map(|&(region, ..)| region);
            let views =
                std::iter::once((SPACE, access.addr)).chain(region.map(|region| (region, addr)));

            for (region, addr) in views {
                let covered = marks
                    .iter()
                    .any(|mark| mark.covers(access.kind, &cpu, region, addr, access.width.bytes()));
                if covered {
                    hits.push(HookHit {
                        kind: access.kind,
                        cpu: cpu.clone(),
                        region: region.into(),
                        addr,
                        value: access.value,
                        width: access.width,
                        source: AccessSource::Cpu(cpu.clone()),
                        pc: self.pc,
                    });
                }
            }
        }

        hits
    }
}

fn canonical(addr: u32) -> u32 {
    match addr >> 24 {
        5 | 7 => (addr & 0xFF00_0000) | (addr & 0x3FF),
        6 => 0x0600_0000 | ppu::vram_offset(addr) as u32,
        8..=13 => 0x0800_0000 | (addr & 0x01FF_FFFF),
        _ => addr,
    }
}

pub(super) struct TrackedBus<'a, 'b> {
    pub bus: CpuBus<'a>,
    pub capture: &'b mut Capture,
}

impl TrackedBus<'_, '_> {
    fn record(&mut self, addr: u32, value: u32, width: AccessWidth, kind: BreakpointKind) {
        self.capture.record(Access {
            addr,
            value,
            width,
            kind,
        });
    }
}

impl Bus for TrackedBus<'_, '_> {
    fn read8(&mut self, addr: u32) -> u8 {
        let value = self.bus.read8(addr);
        self.record(addr, value.into(), AccessWidth::Byte, BreakpointKind::Read);
        value
    }

    fn read16(&mut self, addr: u32) -> u16 {
        let value = self.bus.read16(addr);
        self.record(
            addr,
            value.into(),
            AccessWidth::Halfword,
            BreakpointKind::Read,
        );
        value
    }

    fn read32(&mut self, addr: u32) -> u32 {
        let value = self.bus.read32(addr);
        self.record(addr, value, AccessWidth::Word, BreakpointKind::Read);
        value
    }

    fn write8(&mut self, addr: u32, value: u8) {
        self.bus.write8(addr, value);
        self.record(addr, value.into(), AccessWidth::Byte, BreakpointKind::Write);
    }

    fn write16(&mut self, addr: u32, value: u16) {
        self.bus.write16(addr, value);
        self.record(
            addr,
            value.into(),
            AccessWidth::Halfword,
            BreakpointKind::Write,
        );
    }

    fn write32(&mut self, addr: u32, value: u32) {
        self.bus.write32(addr, value);
        self.record(addr, value, AccessWidth::Word, BreakpointKind::Write);
    }

    fn fetch16(&mut self, addr: u32) -> u16 {
        self.bus.fetch16(addr)
    }

    fn fetch32(&mut self, addr: u32) -> u32 {
        self.bus.fetch32(addr)
    }
}
