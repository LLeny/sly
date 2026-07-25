mod addressing;
mod decoder;
pub mod disasm;
mod interpreter;
mod operand;
mod timing;

pub use disasm::disassemble_at;

use crate::Snes;
#[cfg(feature = "debug")]
use crate::callstack::CallFrame;
use crate::mmio::{Address, MemoryMap, map};
use interpreter::Interpreter;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    #[default]
    Running,
    Waiting,
    Stopped,
}

#[derive(Serialize, Deserialize)]
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
    pub execution: ExecutionState,

    #[cfg(feature = "debug")]
    #[serde(skip)]
    pub call_stack: Vec<CallFrame>,
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
            execution: ExecutionState::Running,

            #[cfg(feature = "debug")]
            call_stack: Vec::new(),
        }
    }

    /// The M/X width bits are hardwired to 1 and the stack pointer is confined
    /// to page one in emulation mode.
    pub fn normalize_emulation(&mut self) {
        if self.emulation {
            self.flags |= 0x30;
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
}

impl<M: MemoryMap> decoder::DisasmCtx for CodeReader<'_, M> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.snes.peek8(Address::new(self.bank, addr as u16))
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

/// Whether a branch with `mnemonic` redirects under the given flags.
pub(crate) fn branch_taken(flags: &Flags, mnemonic: decoder::Mnemonic) -> bool {
    use decoder::Mnemonic as M;

    match mnemonic {
        M::Bra | M::Brl => true,
        M::Beq => flags.zero(),
        M::Bne => !flags.zero(),
        M::Bcs => flags.carry(),
        M::Bcc => !flags.carry(),
        M::Bmi => flags.negative(),
        M::Bpl => !flags.negative(),
        M::Bvs => flags.overflow(),
        M::Bvc => !flags.overflow(),
        _ => unreachable!("branch mnemonic"),
    }
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

    let reader = CodeReader { snes, bank };
    let arg = operand::fetch_arg(inst, mnemonic, form, pc as u64, &reader);
    addressing::resolve(snes, form, arg).map(|(lo, _)| lo)
}

/// Execute one instruction and return the CPU cycles it took.
pub fn step<M: MemoryMap>(snes: &mut Snes<M>) -> u64 {
    let cycles = match snes.cpu.execution {
        ExecutionState::Stopped => self::idle_cycles(snes),
        _ if snes.irq.take_nmi() => {
            snes.cpu.execution = ExecutionState::Running;
            self::service_vectored(snes, 0xFFEA, 0xFFFA);
            timing::interrupt_cycles(&snes.cpu)
        }
        _ if snes.irq.irq_pending() && !snes.cpu.flags.irq_disable() => {
            snes.cpu.execution = ExecutionState::Running;
            self::service_vectored(snes, 0xFFEE, 0xFFFE);
            timing::interrupt_cycles(&snes.cpu)
        }
        ExecutionState::Waiting if snes.irq.irq_pending() => {
            snes.cpu.execution = ExecutionState::Running;
            self::execute_instruction(snes)
        }
        ExecutionState::Waiting => self::idle_cycles(snes),
        ExecutionState::Running => self::execute_instruction(snes),
    };

    snes.mmu.clocks += map::WAITSTATES_FAST * cycles;

    cycles
}

fn idle_cycles<M: MemoryMap>(snes: &Snes<M>) -> u64 {
    let clocks = snes.scheduler.next_deadline().map_or(0, |deadline| {
        deadline.saturating_sub(snes.scheduler.cycles())
    });
    clocks.div_ceil(map::WAITSTATES_FAST).max(1)
}

fn execute_instruction<M: MemoryMap>(snes: &mut Snes<M>) -> u64 {
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
        let reader = CodeReader { snes: &*snes, bank };
        operand::fetch_arg(inst, mnemonic, form, pc as u64, &reader)
    };

    for i in 1..len as u16 {
        let addr = Address::new(bank, pc.wrapping_add(i));
        snes.mmu.clocks += snes.mmu.waitstates(addr);
    }

    if let Some((ptr, bytes)) = addressing::pointer_fetch(snes, form, arg) {
        for i in 0..bytes as u16 {
            snes.mmu.clocks += snes.mmu.waitstates(ptr.wrapping_add(i));
        }
    }

    let cycles = timing::cycles(snes, mnemonic, form, arg);

    #[cfg(feature = "debug")]
    snes.mmu.access.resume();

    let mut vm = Interpreter::new(&mut *snes, len);
    vm.exec(mnemonic, form, arg);
    vm.commit();

    #[cfg(feature = "debug")]
    snes.mmu.access.pause();

    cycles
}

pub(crate) fn service_vectored<M: MemoryMap>(snes: &mut Snes<M>, native: u16, emulation: u16) {
    let mut vm = Interpreter::new(snes, 0);
    vm.service_interrupt(native, emulation);
    vm.commit();
}

#[chapa::bitfield(u8, order = lsb0)]
#[derive(Debug)]
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

crate::macros::impl_bitfield_serde!(Flags,);
