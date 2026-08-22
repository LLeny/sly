mod addressing;
mod decoder;
pub mod disasm;
mod interpreter;
mod operand;
mod timing;

pub use disasm::disassemble_at;

#[cfg(feature = "debug")]
use crate::callstack::CallFrame;
use crate::mmio::{Address, MemoryMap, map};
use crate::{Bus, Snes};
use interpreter::Interpreter;
use serde::{Deserialize, Serialize};

pub(crate) trait CpuBus {
    fn read8(&mut self, addr: Address) -> u8;
    fn write8(&mut self, addr: Address, value: u8);
    fn peek8(&self, addr: Address) -> u8;

    fn read16(&mut self, addr: Address) -> u16 {
        let lo = self.read8(addr) as u16;
        let hi = self.read8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    fn read24(&mut self, addr: Address) -> u32 {
        let lo = self.read16(addr) as u32;
        let hi = self.read8(addr.wrapping_add(2)) as u32;
        lo | (hi << 16)
    }

    fn peek16(&self, addr: Address) -> u16 {
        let lo = self.peek8(addr) as u16;
        let hi = self.peek8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    fn peek24(&self, addr: Address) -> u32 {
        let lo = self.peek16(addr) as u32;
        let hi = self.peek8(addr.wrapping_add(2)) as u32;
        lo | (hi << 16)
    }
}

impl<M: MemoryMap> CpuBus for Bus<M> {
    fn read8(&mut self, addr: Address) -> u8 {
        Bus::read8(self, addr)
    }

    fn write8(&mut self, addr: Address, value: u8) {
        Bus::write8(self, addr, value);
    }

    fn peek8(&self, addr: Address) -> u8 {
        Bus::peek8(self, addr)
    }
}

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

    #[cfg(feature = "debug")]
    #[serde(skip)]
    pub interrupts: InterruptCounters,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(feature = "debug")]
#[derive(Default, Clone, Copy)]
pub struct InterruptCounters {
    pub nmi: u64,
    pub irq: u64,
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

            #[cfg(feature = "debug")]
            interrupts: InterruptCounters::default(),
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

pub(crate) struct CodeReader<'a, B: CpuBus> {
    pub cpu: &'a Cpu,
    pub bus: &'a B,
    pub bank: u8,
}

impl<B: CpuBus> decoder::DisasmCtx for CodeReader<'_, B> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.bus.peek8(Address::new(self.bank, addr as u16))
    }

    fn symbol(&self, addr: u64) -> Option<(String, u64)> {
        crate::mmio::symbol(addr as u16).map(|name| (name.to_string(), 0))
    }

    /// The decoder's `m`/`x` width selectors.
    fn mode(&self, name: &str) -> u64 {
        match name {
            "m" => !self.cpu.acc_wide() as u64,
            "x" => !self.cpu.index_wide() as u64,
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

struct Fetched {
    mnemonic: decoder::Mnemonic,
    form: decoder::Form,
    arg: operand::Arg,
    len: u8,
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

    let reader = CodeReader {
        cpu: &snes.cpu,
        bus: &snes.bus,
        bank,
    };
    let arg = operand::fetch_arg(inst, mnemonic, form, pc as u64, &reader);
    addressing::resolve(&snes.cpu, &snes.bus, form, arg).map(|(lo, _)| lo)
}

/// Execute one instruction and return the CPU cycles it took.
pub fn step<M: MemoryMap>(snes: &mut Snes<M>) -> u64 {
    let Snes { cpu, bus } = snes;
    self::step_main(cpu, bus)
}

fn step_main<M: MemoryMap>(cpu: &mut Cpu, snes: &mut Bus<M>) -> u64 {
    match cpu.execution {
        ExecutionState::Stopped => self::idle(snes),
        _ if snes.irq.take_nmi() => self::service_interrupt(cpu, snes, 0xFFEA, 0xFFFA),
        _ if snes.irq.irq_pending() && !cpu.flags.irq_disable() => {
            self::service_interrupt(cpu, snes, 0xFFEE, 0xFFFE)
        }
        ExecutionState::Waiting if snes.irq.irq_pending() => {
            cpu.execution = ExecutionState::Running;
            self::execute_instruction(cpu, snes)
        }
        ExecutionState::Waiting => self::idle(snes),
        ExecutionState::Running => self::execute_instruction(cpu, snes),
    }
}

fn charge<M: MemoryMap>(snes: &mut Bus<M>, cycles: u64) -> u64 {
    snes.mmu.clocks += map::WAITSTATES_FAST * cycles;
    cycles
}

fn idle<M: MemoryMap>(snes: &mut Bus<M>) -> u64 {
    let cycles = self::idle_cycles(snes);
    self::charge(snes, cycles)
}

fn service_interrupt<M: MemoryMap>(
    cpu: &mut Cpu,
    snes: &mut Bus<M>,
    native: u16,
    emulation: u16,
) -> u64 {
    #[cfg(feature = "debug")]
    if native == 0xFFEA {
        cpu.interrupts.nmi += 1;
    } else {
        cpu.interrupts.irq += 1;
    }

    cpu.execution = ExecutionState::Running;
    self::service_vectored(cpu, snes, native, emulation);
    let cycles = timing::interrupt_cycles(cpu);
    self::charge(snes, cycles)
}

fn idle_cycles<M: MemoryMap>(snes: &Bus<M>) -> u64 {
    let clocks = snes.scheduler.next_deadline().map_or(0, |deadline| {
        deadline.saturating_sub(snes.scheduler.cycles())
    });
    clocks.div_ceil(map::WAITSTATES_FAST).max(1)
}

fn execute_instruction<M: MemoryMap>(cpu: &mut Cpu, snes: &mut Bus<M>) -> u64 {
    let bank = cpu.program_bank;
    let pc = cpu.pc;
    let fetched = self::fetch(cpu, snes);

    for i in 1..fetched.len as u16 {
        let addr = Address::new(bank, pc.wrapping_add(i));
        snes.mmu.clocks += snes.mmu.waitstates(addr);
    }

    if let Some((ptr, bytes)) = addressing::pointer_fetch(cpu, fetched.form, fetched.arg) {
        for i in 0..bytes as u16 {
            snes.mmu.clocks += snes.mmu.waitstates(ptr.wrapping_add(i));
        }
    }

    let cycles = self::charge(
        snes,
        timing::cycles(cpu, snes, fetched.mnemonic, fetched.form, fetched.arg),
    );

    #[cfg(feature = "debug")]
    snes.mmu.access.resume();

    self::execute_fetched(cpu, snes, fetched);

    #[cfg(feature = "debug")]
    snes.mmu.access.pause();

    cycles
}

fn fetch<B: CpuBus>(cpu: &Cpu, bus: &mut B) -> Fetched {
    let bank = cpu.program_bank;
    let pc = cpu.pc;
    let combo = self::mode_selectors(cpu);
    let opcode = bus.read8(Address::new(bank, pc));
    let Decoded {
        inst,
        id,
        mnemonic,
        form,
    } = self::decode(opcode, combo);
    let len = decoder::stream_len_of(combo, id);
    let reader = CodeReader {
        cpu,
        bus: &*bus,
        bank,
    };
    let arg = operand::fetch_arg(inst, mnemonic, form, pc as u64, &reader);

    Fetched {
        mnemonic,
        form,
        arg,
        len,
    }
}

fn execute_fetched<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, fetched: Fetched) {
    let mut vm = Interpreter::new(cpu, bus, fetched.len);
    vm.exec(fetched.mnemonic, fetched.form, fetched.arg);
    vm.commit();
}

pub(crate) fn step_on_bus<B: CpuBus>(cpu: &mut Cpu, bus: &mut B) -> u64 {
    if cpu.execution != ExecutionState::Running {
        return 1;
    }

    let fetched = self::fetch(cpu, bus);
    let cycles = timing::cycles(cpu, bus, fetched.mnemonic, fetched.form, fetched.arg);
    self::execute_fetched(cpu, bus, fetched);
    cycles
}

pub(crate) fn service_vectored<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, native: u16, emulation: u16) {
    let mut vm = Interpreter::new(cpu, bus, 0);
    vm.service_interrupt(native, emulation);
    vm.commit();
}

pub(crate) fn service_interrupt_at<B: CpuBus>(cpu: &mut Cpu, bus: &mut B, target: u16) -> u64 {
    cpu.execution = ExecutionState::Running;
    let cycles = timing::interrupt_cycles(cpu);
    let mut vm = Interpreter::new(cpu, bus, 0);
    vm.service_interrupt_at(target);
    vm.commit();
    cycles
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
