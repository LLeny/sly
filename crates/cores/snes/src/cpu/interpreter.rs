use super::decoder;
use crate::{Snes, cpu::Flags};
use decoder::{Branches, FlagMask, FlagOps, Instruction, Load, RegStep, Store, SwTrap};

pub(crate) struct InterpreterView<'a> {
    snes: &'a mut Snes,
    next_bank: u8,
    next_pc: u16,
}

impl<'a> InterpreterView<'a> {
    pub(crate) fn new(snes: &'a mut Snes, len: u8) -> Self {
        let cpu = &snes.cpu;
        let next_bank = cpu.program_bank;
        let next_pc = cpu.pc.wrapping_add(len as u16);

        Self {
            snes,
            next_bank,
            next_pc,
        }
    }

    /// The committed NIA (program bank, offset) after the op has run.
    pub(crate) fn next(&self) -> (u8, u16) {
        (self.next_bank, self.next_pc)
    }

    /// Redirect execution within the current program bank.
    fn jump(&mut self, pc: u16) {
        self.next_pc = pc;
    }

    /// Redirect execution to another program bank.
    fn jump_long(&mut self, bank: u8, pc: u16) {
        self.next_bank = bank;
        self.next_pc = pc;
    }

    /// Take a relative branch.
    fn branch(&mut self, offset: i16) {
        self.next_pc = self.next_pc.wrapping_add(offset as u16);
    }

    /// Effective address of a 16bit operand within the current data bank.
    fn data_ea(&self, addr: u16) -> u32 {
        ((self.snes.cpu.data_bank as u32) << 16) | addr as u32
    }

    /// Push a byte onto the stack.
    fn push8(&mut self, value: u8) {
        let sp = self.snes.cpu.stack_pointer;
        self.snes.write(sp as u32, value);
        self.snes.cpu.stack_pointer = sp.wrapping_sub(1);
    }

    /// Push a word onto the stack.
    fn push16(&mut self, value: u16) {
        self.push8((value >> 8) as u8);
        self.push8(value as u8);
    }

    /// Shared interrupt entry sequence.
    fn service_interrupt(&mut self, native_vector: u16, emu_vector: u16) {
        let (program_bank, return_pc) = self.next();
        let emulation = self.snes.cpu.emulation;

        if !emulation {
            self.push8(program_bank);
        }

        self.push16(return_pc);
        self.push8(self.snes.cpu.flags.raw());

        self.snes.cpu.flags.set_irq_disable(true);
        self.snes.cpu.flags.set_decimal(false);
        self.snes.cpu.program_bank = 0;

        let vector = if emulation { emu_vector } else { native_vector };
        let target = self.snes.read16(0, vector);
        self.jump_long(0, target);
    }
}

impl InterpreterView<'_> {
    pub(crate) fn exec(&mut self, inst: Instruction) {
        match inst {
            Instruction::Nop => {}

            Instruction::RegStep(op) => self.reg_step(op),
            Instruction::FlagOps(op) => self.flag_ops(op),
            Instruction::FlagMask(op) => self.flag_mask(op),
            Instruction::Branches(op) => self.branches(op),
            Instruction::SwTrap(op) => self.sw_trap(op),
            Instruction::Store(op) => self.store(op),
            Instruction::Load(op) => self.load(op),

            Instruction::JmpAbs { addr } => self.jump(addr),
            Instruction::JmpAbl { addr } => self.jump_long((addr >> 16) as u8, addr as u16),
            Instruction::BrlRell { disp } => self.branch(disp),
            Instruction::Xce => self.xce(),

            other => todo!("Implement instruction {other:?}"),
        }
    }

    fn reg_step(&mut self, op: RegStep) {
        let cpu = &mut self.snes.cpu;
        match op {
            RegStep::Inx => cpu.index_x = cpu.index_x.wrapping_add(1),
            RegStep::Dex => cpu.index_x = cpu.index_x.wrapping_sub(1),
            RegStep::Iny => cpu.index_y = cpu.index_y.wrapping_add(1),
            RegStep::Dey => cpu.index_y = cpu.index_y.wrapping_sub(1),
            RegStep::IncA => cpu.acc = cpu.acc.wrapping_add(1),
            RegStep::DecA => cpu.acc = cpu.acc.wrapping_sub(1),
        }
    }

    fn flag_ops(&mut self, op: FlagOps) {
        let flags = &mut self.snes.cpu.flags;
        match op {
            FlagOps::Clc => flags.set_carry(false),
            FlagOps::Sec => flags.set_carry(true),
            FlagOps::Cli => flags.set_irq_disable(false),
            FlagOps::Sei => flags.set_irq_disable(true),
            FlagOps::Cld => flags.set_decimal(false),
            FlagOps::Sed => flags.set_decimal(true),
            FlagOps::Clv => flags.set_overflow(false),
        }
    }

    fn flag_mask(&mut self, op: FlagMask) {
        let cpu = &mut self.snes.cpu;
        let raw = cpu.flags.raw();
        cpu.flags = Flags::from_raw(match op {
            FlagMask::RepImm { mask } => raw & !mask,
            FlagMask::SepImm { mask } => raw | mask,
        });

        if cpu.flags.x() {
            cpu.index_x &= 0x00ff;
            cpu.index_y &= 0x00ff;
        }
    }

    fn branches(&mut self, op: Branches) {
        let flags = self.snes.cpu.flags;
        let (taken, disp) = match op {
            Branches::BraRel { disp } => (true, disp),
            Branches::BeqRel { disp } => (flags.zero_(), disp),
            Branches::BneRel { disp } => (!flags.zero_(), disp),
            Branches::BcsRel { disp } => (flags.carry(), disp),
            Branches::BccRel { disp } => (!flags.carry(), disp),
            Branches::BmiRel { disp } => (flags.negative(), disp),
            Branches::BplRel { disp } => (!flags.negative(), disp),
            Branches::BvsRel { disp } => (flags.overflow(), disp),
            Branches::BvcRel { disp } => (!flags.overflow(), disp),
        };

        if taken {
            self.branch(disp as i16);
        }
    }

    fn sw_trap(&mut self, op: SwTrap) {
        const BRK_VECTOR_NATIVE: u16 = 0xFFE6;
        const BRK_VECTOR_EMULATION: u16 = 0xFFFE;
        const COP_VECTOR_NATIVE: u16 = 0xFFE4;
        const COP_VECTOR_EMULATION: u16 = 0xFFF4;

        let (native_vector, emu_vector) = match op {
            SwTrap::BrkImm { .. } => (BRK_VECTOR_NATIVE, BRK_VECTOR_EMULATION),
            SwTrap::CopImm { .. } => (COP_VECTOR_NATIVE, COP_VECTOR_EMULATION),
        };
        self.service_interrupt(native_vector, emu_vector);
    }

    fn store(&mut self, op: Store) {
        match op {
            Store::StzAbs { addr } => {
                let ea = self.data_ea(addr);
                self.snes.write(ea, 0);

                if self.snes.cpu.flags.is_acc_16bits() {
                    self.snes.write(ea + 1, 0);
                }
            }
            Store::StaAbs { addr } => {
                let ea = self.data_ea(addr);
                let value = self.snes.cpu.acc as u8;
                self.snes.write(ea, value);

                if self.snes.cpu.flags.is_acc_16bits() {
                    let value_hi = (self.snes.cpu.acc >> 8) as u8;
                    self.snes.write(ea + 1, value_hi);
                }
            }
            other => todo!("Implement store instruction {other:?}"),
        }
    }

    fn load(&mut self, op: Load) {
        match op {
            Load::LdaImmM8 { imm } => {
                let ea = self.data_ea(imm.into());
                let value = self.snes.read(ea);

                if self.snes.cpu.flags.is_acc_16bits() {
                    let value_hi = self.snes.read(ea + 1);
                    self.snes.cpu.acc = (value_hi as u16) << 8 | value as u16;
                } else {
                    self.snes.cpu.acc = value as u16;
                }
            }
            other => todo!("Implement load instruction {other:?}"),
        }
    }

    fn xce(&mut self) {
        let cpu = &mut self.snes.cpu;
        let carry = cpu.flags.carry();
        cpu.flags.set_carry(cpu.emulation);
        cpu.emulation = carry;

        if cpu.emulation {
            tracing::error!("Entering emulation mode, weird?");
        }
    }
}
