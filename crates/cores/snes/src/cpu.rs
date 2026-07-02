mod addressing;
mod decoder;
pub mod disasm;
mod interpreter;
mod operand;

pub use disasm::disassemble_at;

use crate::Snes;
use crate::mmio::{Address, MemoryMap};
use interpreter::Interpreter;

pub struct Cpu {
    pub acc: u16,
    pub index_x: u16,
    pub index_y: u16,
    pub data_bank: u8,
    pub direct_page: u16,
    pub program_bank: u8,
    pub flags: Flags,
    pub pc: u16,
    pub stack_pointer: u16,
    pub emulation: bool,
}

impl Cpu {
    pub fn new(addr: u16) -> Self {
        Self {
            pc: addr,
            acc: 0,
            index_x: 0,
            index_y: 0,
            data_bank: 0,
            direct_page: 0,
            program_bank: 0,
            flags: Flags::default(),
            stack_pointer: 0x01ff,
            emulation: true,
        }
    }

    /// The M/X width bits are hardwired to 1 and the stack pointer is confined
    /// to page one in emulation mode.
    pub fn normalize_emulation(&mut self) {
        if self.emulation {
            self.flags = Flags::from_raw(self.flags.raw() | 0x30);
            self.stack_pointer = self.confine_sp(self.stack_pointer);
        }
    }

    /// Confine a stack pointer to page one while in emulation mode.
    pub fn confine_sp(&self, sp: u16) -> u16 {
        if self.emulation {
            0x0100 | (sp & 0x00ff)
        } else {
            sp
        }
    }

    /// Clear the index register high bytes while the X flag selects 8bit width.
    pub fn narrow_index(&mut self) {
        if self.flags.x() {
            self.index_x &= 0x00ff;
            self.index_y &= 0x00ff;
        }
    }

    /// Accumulator/memory operations are 16bit. Emulation mode forces 8bit
    /// regardless of the M bit.
    pub fn acc_wide(&self) -> bool {
        !self.emulation && self.flags.is_acc_16bits()
    }

    /// Index registers are 16bit. Emulation mode forces 8bit regardless of the
    /// X bit.
    pub fn index_wide(&self) -> bool {
        !self.emulation && self.flags.is_index_16bits()
    }
}

pub(crate) struct CodeReader<'a, M: MemoryMap> {
    pub snes: &'a Snes<M>,
    pub bank: u8,
    pub peek: bool,
}

impl<M: MemoryMap> decoder::DisasmCtx for CodeReader<'_, M> {
    fn read_u8(&self, addr: u64) -> u8 {
        let addr = Address::new(self.bank, addr as u16);
        if self.peek {
            self.snes.peek8(addr)
        } else {
            self.snes.read8(addr)
        }
    }

    fn symbol(&self, addr: u64) -> Option<(String, u64)> {
        crate::mmio::symbol(addr as u16).map(|name| (name.to_string(), 0))
    }

    /// The decoder's `m`/`x` width selectors.
    fn mode(&self, name: &str) -> u64 {
        let cpu = &self.snes.cpu;
        match name {
            "m" => !cpu.acc_wide() as u64,
            "x" => !cpu.index_wide() as u64,
            _ => 0,
        }
    }
}

/// Pack the M/X width selectors the decoder needs.
pub(crate) fn mode_selectors(cpu: &Cpu) -> usize {
    decoder::pack_modes(!cpu.acc_wide() as u64, !cpu.index_wide() as u64)
}

pub(crate) struct Decoded {
    pub inst: decoder::Instruction,
    pub id: usize,
    pub mnemonic: decoder::Mnemonic,
    pub form: decoder::Form,
}

/// Resolve an opcode byte to its instruction, id, mnemonic and form.
pub(crate) fn decode(opcode: u8, combo: usize) -> Decoded {
    let inst = decoder::Instruction(opcode);
    let id = inst.opcode_in(combo);
    Decoded {
        inst,
        id,
        mnemonic: decoder::OPCODE_MNEMONIC[id],
        form: decoder::OPCODE_FORM[id],
    }
}

/// The effective data address of the instruction at `pc`.
pub fn effective_address<M: MemoryMap>(snes: &Snes<M>, pc: u16) -> Option<Address> {
    use decoder::Form as F;

    let combo = self::mode_selectors(&snes.cpu);
    let bank = snes.cpu.program_bank;
    let opcode = snes.peek8(Address::new(bank, pc));
    let Decoded {
        inst,
        mnemonic,
        form,
        ..
    } = self::decode(opcode, combo);

    if matches!(form, F::Dp | F::Abs | F::Abl) {
        return None;
    }

    let reader = CodeReader {
        snes,
        bank,
        peek: true,
    };
    let arg = operand::fetch_arg(inst, mnemonic, form, pc as u64, &reader);
    addressing::resolve(snes, form, arg).map(|(lo, _)| lo)
}

pub fn step<M: MemoryMap>(snes: &mut Snes<M>) {
    let (bank, pc, combo) = {
        let cpu = &snes.cpu;
        (cpu.program_bank, cpu.pc, self::mode_selectors(cpu))
    };

    let opcode = snes.read8(Address::new(bank, pc));
    let Decoded {
        inst,
        id,
        mnemonic,
        form,
    } = self::decode(opcode, combo);
    let len = decoder::stream_len_of(combo, id);

    let arg = {
        let reader = CodeReader {
            snes: &*snes,
            bank,
            peek: false,
        };
        operand::fetch_arg(inst, mnemonic, form, pc as u64, &reader)
    };

    #[cfg(feature = "debug")]
    snes.mmu.access.resume();

    let mut vm = Interpreter::new(&mut *snes, len);
    vm.exec(mnemonic, form, arg);
    let (next_bank, next_pc) = vm.next();

    #[cfg(feature = "debug")]
    snes.mmu.access.pause();

    snes.cpu.program_bank = next_bank;
    snes.cpu.pc = next_pc;
    snes.cpu.normalize_emulation();
    snes.cpu.narrow_index();
}

#[chapa::bitfield(u8, order = lsb0)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Flags {
    #[bits(0, alias = "c")]
    pub carry: bool,

    #[bits(1, alias = "z")]
    pub zero: bool,

    #[bits(2, alias = "i")]
    pub irq_disable: bool,

    #[bits(3, alias = "d")]
    pub decimal: bool,

    #[bits(4, alias = "x", default = true)]
    pub index_register_size: bool,

    #[bits(5, alias = "m", default = true)]
    pub accumulator_register_size: bool,

    #[bits(6, alias = "v")]
    pub overflow: bool,

    #[bits(7, alias = "n")]
    pub negative: bool,
}

impl Flags {
    pub fn is_acc_16bits(&self) -> bool {
        !self.m()
    }

    pub fn is_index_16bits(&self) -> bool {
        !self.x()
    }
}
