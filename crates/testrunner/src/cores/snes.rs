use snes::apu::Apu;
use snes::apu::spc700::{self, Flags as PswFlags};
use snes::cpu::Flags;
use snes::mmio::Address;
use snes::{Cartridge, Snes};

use crate::harness::{Regs, State, TestCore};

const FLAT_BANK: u8 = 0x7E;
const EMU_WIDTH_BITS: u8 = 0x30;

pub struct SnesCpu {
    snes: Snes,
}

impl SnesCpu {
    pub fn new() -> Self {
        let rom = vec![0u8; 0x1_0000];
        let cart = Cartridge::new(rom).unwrap();
        Self {
            snes: Snes::new(cart),
        }
    }
}

impl TestCore for SnesCpu {
    fn load(&mut self, initial: &State) {
        self.snes.mmu.reset();
        for &(addr, value) in &initial.ram {
            self.snes.write8(Address::new(FLAT_BANK, addr), value);
        }

        let cpu = &mut self.snes.cpu;
        cpu.pc = initial.pc;

        cpu.stack_pointer = 0x0100 | initial.sp as u16;
        cpu.acc = initial.a as u16;
        cpu.index_x = initial.x as u16;
        cpu.index_y = initial.y as u16;

        cpu.flags = Flags::from_raw(initial.psw | EMU_WIDTH_BITS);
        cpu.data_bank = FLAT_BANK;
        cpu.program_bank = FLAT_BANK;
        cpu.direct_page = 0;
    }

    fn step(&mut self) {
        snes::cpu::step(&mut self.snes);
    }

    fn regs(&self) -> Regs {
        let cpu = &self.snes.cpu;
        Regs {
            pc: cpu.pc,
            sp: cpu.stack_pointer as u8,
            a: cpu.acc as u8,
            x: cpu.index_x as u8,
            y: cpu.index_y as u8,
            psw: cpu.flags.raw(),
        }
    }

    fn peek(&self, addr: u16) -> u8 {
        self.snes.peek8(Address::new(FLAT_BANK, addr))
    }

    fn ignore_flags(&self) -> u8 {
        EMU_WIDTH_BITS
    }
}

pub struct Spc700 {
    apu: Apu,
}

impl Spc700 {
    pub fn new() -> Self {
        Self { apu: Apu::new() }
    }
}

impl TestCore for Spc700 {
    fn load(&mut self, initial: &State) {
        self.apu = Apu::new();
        for &(addr, value) in &initial.ram {
            self.apu.write(addr, value);
        }

        let spc = &mut self.apu.spc700;
        spc.pc = initial.pc;
        spc.sp = initial.sp;
        spc.a = initial.a;
        spc.x = initial.x;
        spc.y = initial.y;
        spc.psw = PswFlags::from_raw(initial.psw);
    }

    fn step(&mut self) {
        spc700::step(&mut self.apu);
    }

    fn regs(&self) -> Regs {
        let spc = &self.apu.spc700;
        Regs {
            pc: spc.pc,
            sp: spc.sp,
            a: spc.a,
            x: spc.x,
            y: spc.y,
            psw: spc.psw.raw(),
        }
    }

    fn peek(&self, addr: u16) -> u8 {
        self.apu.peek(addr)
    }
}
