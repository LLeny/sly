use crate::Snes;
use crate::mmio::MemoryMap;
use crate::mmio::device::{Access, IoRegister, RegEntry};
use crate::sa1::regs::{
    BMAPS, BWPA, CIWP, CharacterConversionDmaParameters, ROM_BANK_END, ROM_BANK_START, SBWE, SIC,
    SIE, SIWP, Sa1CpuControl, SnesFlags, VECTOR_END, VECTOR_START,
};

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![
        Ccnt::entry(),
        Sie::entry(),
        Sic::entry(),
        Vectors::entry(),
        RomBanks::entry(),
        Bmaps::entry(),
        Sbwe::entry(),
        Bwpa::entry(),
        Siwp::entry(),
        Ciwp::entry(),
        Cdma::entry(),
        Sfr::entry(),
    ]
}

struct RomBanks;

impl IoRegister for RomBanks {
    const START: u16 = ROM_BANK_START;
    const END: u16 = ROM_BANK_END;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, reg: u16, value: u8) {
        snes.sa1.memory.set_rom_bank(reg, value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, reg: u16) -> u8 {
        snes.sa1.memory.rom_bank_raw(reg)
    }
}

struct Bmaps;

impl IoRegister for Bmaps {
    const START: u16 = BMAPS;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.memory.set_bwram_snes_block(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.memory.bwram_snes_block_raw()
    }
}

struct Sbwe;

impl IoRegister for Sbwe {
    const START: u16 = SBWE;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.memory.set_bwram_snes_write_enable(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.memory.bwram_snes_write_enable_raw()
    }
}

struct Bwpa;

impl IoRegister for Bwpa {
    const START: u16 = BWPA;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.memory.set_bwram_protected_area(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.memory.bwram_protected_area_raw()
    }
}

struct Ccnt;

impl IoRegister for Ccnt {
    const START: u16 = Sa1CpuControl::ADDRESS;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.set_ccnt(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.ccnt.raw()
    }
}

struct Sie;

impl IoRegister for Sie {
    const START: u16 = SIE;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.set_sie(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.sie.raw()
    }
}

struct Sic;

impl IoRegister for Sic {
    const START: u16 = SIC;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.clear_snes_interrupts(value);
    }

    fn peek<M: MemoryMap>(_snes: &Snes<M>, _reg: u16) -> u8 {
        0
    }
}

struct Vectors;

impl IoRegister for Vectors {
    const START: u16 = VECTOR_START;
    const END: u16 = VECTOR_END;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, reg: u16, value: u8) {
        snes.sa1.set_vector_byte(reg, value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, reg: u16) -> u8 {
        snes.sa1.vector_byte(reg)
    }
}

struct Siwp;

impl IoRegister for Siwp {
    const START: u16 = SIWP;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.set_siwp(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.siwp.raw()
    }
}

struct Ciwp;

impl IoRegister for Ciwp {
    const START: u16 = CIWP;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.set_ciwp(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.ciwp.raw()
    }
}

struct Cdma;

impl IoRegister for Cdma {
    const START: u16 = CharacterConversionDmaParameters::ADDRESS;
    const READ: Access = Access::Ignored;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        snes.sa1.set_cdma(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.cdma.raw()
    }
}

struct Sfr;

impl IoRegister for Sfr {
    const START: u16 = SnesFlags::ADDRESS;
    const WRITE: Access = Access::Unmapped;

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        snes.sa1.sfr().raw()
    }
}
