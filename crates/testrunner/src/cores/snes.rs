use snes::apu::spc700::{self, Flags as PswFlags};
use snes::apu::{Apu, ApuMap, Region};
use snes::cpu::Flags;
use snes::mmio::{Address, AddressSpace};
use snes::{Cartridge, MemoryMap, Mmio, Snes};

use crate::harness::{Regs, State, TestCore};

pub struct FlatMap;

impl MemoryMap for FlatMap {
    const WRAM_SIZE: usize = 0x100_0000;

    fn map(&self, _cartridge: &Cartridge, addr: Address) -> AddressSpace {
        AddressSpace::Wram(addr.as_24bit() as usize)
    }
}

pub struct SnesCpu {
    snes: Snes<FlatMap>,
}

impl SnesCpu {
    pub fn new() -> Self {
        let cartridge = Cartridge::new(vec![0; 0x1_0000]).expect("stub cartridge");
        Self {
            snes: Snes::with_mmu(Mmio::with_map(cartridge, FlatMap)),
        }
    }
}

impl TestCore for SnesCpu {
    fn load(&mut self, initial: &State) {
        let cpu = &mut self.snes.cpu;
        cpu.emulation = initial.e != 0;
        cpu.pc = initial.pc;
        cpu.acc = initial.a;
        cpu.index_x = initial.x;
        cpu.index_y = initial.y;
        cpu.flags = Flags::from_raw(initial.p);
        cpu.data_bank = initial.dbr;
        cpu.program_bank = initial.pbr;
        cpu.direct_page = initial.d;
        cpu.stack_pointer = initial.s;
        cpu.normalize_emulation();

        for &(addr, value) in &initial.ram {
            self.snes.write8(Address::from_24bit(addr), value);
        }
    }

    fn step(&mut self) {
        snes::cpu::step(&mut self.snes);
    }

    fn regs(&self) -> Regs {
        let cpu = &self.snes.cpu;
        Regs {
            pc: cpu.pc,
            s: cpu.stack_pointer,
            a: cpu.acc,
            x: cpu.index_x,
            y: cpu.index_y,
            p: cpu.flags.raw(),
            dbr: cpu.data_bank,
            d: cpu.direct_page,
            pbr: cpu.program_bank,
            e: cpu.emulation as u8,
        }
    }

    fn peek(&self, addr: u32) -> u8 {
        self.snes.peek8(Address::from_24bit(addr))
    }
}

struct FlatRam;

impl ApuMap for FlatRam {
    fn region(&self, _addr: u16) -> Region {
        Region::Ram
    }
}

pub struct Spc700 {
    apu: Apu<FlatRam>,
}

impl Spc700 {
    pub fn new() -> Self {
        Self {
            apu: Apu::with_map(FlatRam),
        }
    }
}

impl TestCore for Spc700 {
    fn load(&mut self, initial: &State) {
        for &(addr, value) in &initial.ram {
            self.apu.write(addr as u16, value);
        }

        let spc = &mut self.apu.spc700;
        spc.pc = initial.pc;
        spc.sp = initial.s as u8;
        spc.a = initial.a as u8;
        spc.x = initial.x as u8;
        spc.y = initial.y as u8;
        spc.psw = PswFlags::from_raw(initial.p);
    }

    fn step(&mut self) {
        spc700::step(&mut self.apu);
    }

    fn regs(&self) -> Regs {
        let spc = &self.apu.spc700;
        Regs {
            pc: spc.pc,
            s: spc.sp as u16,
            a: spc.a as u16,
            x: spc.x as u16,
            y: spc.y as u16,
            p: spc.psw.raw(),
            dbr: 0,
            d: 0,
            pbr: 0,
            e: 0,
        }
    }

    fn peek(&self, addr: u32) -> u8 {
        self.apu.peek(addr as u16)
    }
}
