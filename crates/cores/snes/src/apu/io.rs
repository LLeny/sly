use crate::Snes;
use crate::mmio::MemoryMap;
use crate::mmio::device::{IoRegister, RegEntry};
use crate::mmio::regs::{APU_IO_END, APU_IO_START};

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![ApuPorts::entry()]
}

struct ApuPorts;

impl IoRegister for ApuPorts {
    const START: u16 = APU_IO_START;
    const END: u16 = APU_IO_END;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, reg: u16, value: u8) {
        snes.apu
            .cpu_write_port((reg - APU_IO_START) as usize, value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, reg: u16) -> u8 {
        snes.apu.cpu_read_port((reg - APU_IO_START) as usize)
    }
}
