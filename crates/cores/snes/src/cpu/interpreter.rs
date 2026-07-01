use super::decoder;
use crate::Snes;
use crate::cpu::Flags;
use crate::cpu::addressing::{self, AddrMode};
use crate::cpu::decoder::{Alu, Call, Compare, Pop, Push, Shift, Transfer};
use crate::mmio::Address;
use decoder::{Branches, FlagMask, FlagOps, Instruction, Jump, Load, RegStep, Store, SwTrap};

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
    fn set_pc(&mut self, pc: u16) {
        self.next_pc = pc;
    }

    /// Redirect execution to another program bank.
    fn set_pc_long(&mut self, bank: u8, pc: u16) {
        self.next_bank = bank;
        self.next_pc = pc;
    }

    /// Take a relative branch.
    fn branch(&mut self, offset: i16) {
        self.next_pc = self.next_pc.wrapping_add(offset as u16);
    }

    /// Write the low byte of `value` to `ea`, and the high byte to the next
    /// address when the accumulator is in 16bit mode.
    fn write(&mut self, ea: Address, value: u16) {
        if self.snes.cpu.flags.is_acc_16bits() {
            self.snes.write16(ea, value);
        } else {
            self.snes.write8(ea, value as u8);
        }
    }

    /// Read the low byte from `ea`, and the high byte from the next address when
    /// the accumulator is in 16bit mode.
    fn read(&self, ea: Address) -> u16 {
        if self.snes.cpu.flags.is_acc_16bits() {
            self.snes.read16(ea)
        } else {
            self.snes.read8(ea) as u16
        }
    }

    /// Write a result into the accumulator honoring its width.
    fn set_acc(&mut self, value: u16, wide: bool) {
        self.snes.cpu.acc = if wide {
            value
        } else {
            (self.snes.cpu.acc & 0xff00) | (value & 0x00ff)
        };
    }

    /// Load a value into the accumulator and update N/Z, honoring its width.
    fn load_acc(&mut self, value: u16, wide: bool) {
        self.set_acc(value, wide);
        self.set_nz(value, wide);
    }

    /// Push a byte onto the stack.
    fn push8(&mut self, value: u8) {
        let sp = self.snes.cpu.stack_pointer;
        self.snes.write8(Address::new(0, sp), value);
        self.snes.cpu.stack_pointer = sp.wrapping_sub(1);
    }

    /// Push a word onto the stack.
    fn push16(&mut self, value: u16) {
        self.push8((value >> 8) as u8);
        self.push8(value as u8);
    }

    /// Push a register value, honoring its current 8/16bit width.
    fn push_reg(&mut self, value: u16, wide: bool) {
        if wide {
            self.push16(value);
        } else {
            self.push8(value as u8);
        }
    }

    /// Pop a byte from the stack.
    fn pop8(&mut self) -> u8 {
        let sp = self.snes.cpu.stack_pointer.wrapping_add(1);
        self.snes.cpu.stack_pointer = sp;
        self.snes.read8(Address::new(0, sp))
    }

    /// Pop a word from the stack.
    fn pop16(&mut self) -> u16 {
        let lo = self.pop8() as u16;
        let hi = self.pop8() as u16;
        lo | (hi << 8)
    }

    /// Pop a register value, honoring its current 8/16bit width.
    fn pop_reg(&mut self, wide: bool) -> u16 {
        if wide {
            self.pop16()
        } else {
            self.pop8() as u16
        }
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
        let target = self.snes.read16(Address::new(0, vector));
        self.set_pc_long(0, target);
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
            Instruction::Jump(op) => self.jump(op),
            Instruction::Transfer(op) => self.transfer(op),
            Instruction::Alu(op) => self.alu(op),
            Instruction::Call(op) => self.call(op),
            Instruction::Push(op) => self.push(op),
            Instruction::Pop(op) => self.pop(op),
            Instruction::Compare(op) => self.compare(op),
            Instruction::Shift(op) => self.shift(op),

            Instruction::BrlRell { disp } => self.branch(disp),
            Instruction::Xce => self.xce(),
            Instruction::Xba => self.xba(),

            other => todo!("Implement instruction {other:?}"),
        }
    }

    fn reg_step(&mut self, op: RegStep) {
        match op {
            RegStep::Inx => {
                self.snes.cpu.index_x = self.snes.cpu.index_x.wrapping_add(1);
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
            RegStep::Dex => {
                self.snes.cpu.index_x = self.snes.cpu.index_x.wrapping_sub(1);
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
            RegStep::Iny => {
                self.snes.cpu.index_y = self.snes.cpu.index_y.wrapping_add(1);
                self.set_nz(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits());
            }
            RegStep::Dey => {
                self.snes.cpu.index_y = self.snes.cpu.index_y.wrapping_sub(1);
                self.set_nz(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits());
            }
            RegStep::IncA => {
                self.snes.cpu.acc = self.snes.cpu.acc.wrapping_add(1);
                self.set_nz(self.snes.cpu.acc, self.snes.cpu.flags.is_acc_16bits());
            }
            RegStep::DecA => {
                self.snes.cpu.acc = self.snes.cpu.acc.wrapping_sub(1);
                self.set_nz(self.snes.cpu.acc, self.snes.cpu.flags.is_acc_16bits());
            }
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
            Branches::BeqRel { disp } => (flags.zero(), disp),
            Branches::BneRel { disp } => (!flags.zero(), disp),
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

    fn jump(&mut self, op: Jump) {
        match op {
            Jump::JmpAbs { addr } => self.set_pc(addr),
            Jump::JmpAbl { addr } => {
                let target = Address::from_24bit(addr);
                self.set_pc_long(target.bank(), target.address());
            }
            Jump::JmpInd { addr } => {
                let target = self.snes.read16(Address::new(0, addr));
                self.set_pc(target);
            }
            Jump::JmpIax { addr } => {
                let bank = self.snes.cpu.program_bank;
                let ptr = addr.wrapping_add(self.snes.cpu.index_x);
                let target = self.snes.read16(Address::new(bank, ptr));
                self.set_pc(target);
            }
            Jump::JmlIal { addr } => {
                let target =
                    Address::from_24bit(addressing::read24(self.snes, Address::new(0, addr)));
                self.set_pc_long(target.bank(), target.address());
            }
        }
    }

    fn transfer(&mut self, op: Transfer) {
        match op {
            Transfer::Tax => {
                self.snes.cpu.index_x = self.snes.cpu.acc;
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
            Transfer::Tay => {
                self.snes.cpu.index_y = self.snes.cpu.acc;
                self.set_nz(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits());
            }
            Transfer::Tcd => {
                self.snes.cpu.direct_page = self.snes.cpu.acc;
                self.set_nz(self.snes.cpu.direct_page, true);
            }
            Transfer::Tcs => self.snes.cpu.stack_pointer = self.snes.cpu.acc,
            Transfer::Tdc => {
                self.snes.cpu.acc = self.snes.cpu.direct_page;
                self.set_nz(self.snes.cpu.acc, true);
            }
            Transfer::Tsc => {
                self.snes.cpu.acc = self.snes.cpu.stack_pointer;
                self.set_nz(self.snes.cpu.acc, true);
            }
            Transfer::Tsx => {
                self.snes.cpu.index_x = self.snes.cpu.stack_pointer;
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
            Transfer::Txa => {
                self.snes.cpu.acc = self.snes.cpu.index_x;
                self.set_nz(self.snes.cpu.acc, self.snes.cpu.flags.is_acc_16bits());
            }
            Transfer::Txs => self.snes.cpu.stack_pointer = self.snes.cpu.index_x,
            Transfer::Txy => {
                self.snes.cpu.index_y = self.snes.cpu.index_x;
                self.set_nz(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits());
            }
            Transfer::Tya => {
                self.snes.cpu.acc = self.snes.cpu.index_y;
                self.set_nz(self.snes.cpu.acc, self.snes.cpu.flags.is_acc_16bits());
            }
            Transfer::Tyx => {
                self.snes.cpu.index_x = self.snes.cpu.index_y;
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
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
        let (mode, value) = match op {
            Store::StzAbs { addr } => (AddrMode::Abs(addr), 0),
            Store::StaAbs { addr } => (AddrMode::Abs(addr), self.snes.cpu.acc),
            Store::StaAbl { addr } => (AddrMode::Abl(addr), self.snes.cpu.acc),
            Store::StaAlx { addr } => (AddrMode::AlX(addr), self.snes.cpu.acc),
            other => todo!("Implement store instruction {other:?}"),
        };
        let ea = addressing::resolve(self.snes, mode);
        self.write(ea, value);
    }

    fn load(&mut self, op: Load) {
        match op {
            Load::LdaImmM8 { imm } => self.load_acc(imm as u16, false),
            Load::LdaImmM16 { imm } => self.load_acc(imm, true),
            Load::LdaIdly { dp } => {
                let ea = addressing::resolve(self.snes, AddrMode::Idly(dp));
                let wide = self.snes.cpu.flags.is_acc_16bits();
                let value = self.read(ea);
                self.load_acc(value, wide);
            }
            Load::LdxImmX8 { imm } => {
                self.snes.cpu.index_x = imm as u16;
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
            Load::LdxImmX16 { imm } => {
                self.snes.cpu.index_x = imm;
                self.set_nz(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits());
            }
            Load::LdyImmX8 { imm } => {
                self.snes.cpu.index_y = imm as u16;
                self.set_nz(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits());
            }
            Load::LdyImmX16 { imm } => {
                self.snes.cpu.index_y = imm;
                self.set_nz(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits());
            }
            other => todo!("Implement load instruction {other:?}"),
        }
    }

    fn alu(&mut self, op: Alu) {
        match op {
            Alu::SbcImmM16 { imm } => {
                let cpu = &mut self.snes.cpu;

                let borrow = if cpu.flags.carry() { 0 } else { 1 };
                let wide = (cpu.acc as u32)
                    .wrapping_sub(imm as u32)
                    .wrapping_sub(borrow);
                let result = wide as u16;

                cpu.flags.set_carry(wide & 0x1_0000 == 0);
                cpu.flags
                    .set_overflow((cpu.acc ^ imm) & (cpu.acc ^ result) & 0x8000 != 0);
                cpu.flags.set_zero(result == 0);
                cpu.flags.set_negative(result & 0x8000 != 0);

                cpu.acc = result;
            }
            Alu::AdcImmM8 { imm } => {
                let a = self.snes.cpu.acc & 0x00ff;
                let imm = imm as u16;
                let carry = self.snes.cpu.flags.carry() as u16;
                let sum = a + imm + carry;
                let result = sum & 0x00ff;

                let flags = &mut self.snes.cpu.flags;
                flags.set_carry(sum & 0x100 != 0);
                flags.set_overflow((!(a ^ imm) & (a ^ result) & 0x80) != 0);
                flags.set_zero(result == 0);
                flags.set_negative(result & 0x80 != 0);

                self.set_acc(result, false);
            }
            other => todo!("Implement ALU instruction {other:?}"),
        }
    }

    fn call(&mut self, op: Call) {
        match op {
            decoder::Call::JsrAbs { addr } => {
                self.push16(self.next_pc);
                self.set_pc(addr);
            }
            other => todo!("Implement call instruction {other:?}"),
        }
    }

    fn push(&mut self, op: Push) {
        match op {
            Push::Pha => self.push_reg(self.snes.cpu.acc, self.snes.cpu.flags.is_acc_16bits()),
            Push::Phx => {
                self.push_reg(self.snes.cpu.index_x, self.snes.cpu.flags.is_index_16bits())
            }
            Push::Phy => {
                self.push_reg(self.snes.cpu.index_y, self.snes.cpu.flags.is_index_16bits())
            }
            Push::Phb => self.push8(self.snes.cpu.data_bank),
            Push::Phd => self.push16(self.snes.cpu.direct_page),
            Push::Phk => self.push8(self.snes.cpu.program_bank),
            Push::Php => self.push8(self.snes.cpu.flags.raw()),
            Push::PeaAbs { addr } => self.push16(addr),
            Push::PeiIdp { dp } => {
                let ea = addressing::resolve(self.snes, AddrMode::Idp(dp));
                self.push16(ea.address());
            }
            Push::PerRell { disp } => {
                let target = self.next_pc.wrapping_add(disp as u16);
                self.push16(target);
            }
        }
    }

    fn pop(&mut self, op: Pop) {
        match op {
            Pop::Pla => {
                let wide = self.snes.cpu.flags.is_acc_16bits();
                let value = self.pop_reg(wide);
                self.load_acc(value, wide);
            }
            Pop::Plx => {
                let wide = self.snes.cpu.flags.is_index_16bits();
                self.snes.cpu.index_x = self.pop_reg(wide);
                self.set_nz(self.snes.cpu.index_x, wide);
            }
            Pop::Ply => {
                let wide = self.snes.cpu.flags.is_index_16bits();
                self.snes.cpu.index_y = self.pop_reg(wide);
                self.set_nz(self.snes.cpu.index_y, wide);
            }
            Pop::Plb => {
                self.snes.cpu.data_bank = self.pop8();
                self.set_nz(self.snes.cpu.data_bank as u16, false);
            }
            Pop::Pld => {
                self.snes.cpu.direct_page = self.pop16();
                self.set_nz(self.snes.cpu.direct_page, true);
            }
            Pop::Plp => self.snes.cpu.flags = Flags::from_raw(self.pop8()),
        }
    }

    fn compare(&mut self, op: Compare) {
        match op {
            Compare::CmpAbs { addr } => {
                let ea = addressing::resolve(self.snes, AddrMode::Abs(addr));
                let value = self.read(ea);
                let wide = self.snes.cpu.flags.is_acc_16bits();
                self.compare_values(self.snes.cpu.acc, value, wide);
            }
            Compare::CpxImmX16 { imm } => {
                let wide = self.snes.cpu.flags.is_index_16bits();
                self.compare_values(self.snes.cpu.index_x, imm, wide);
            }
            other => todo!("Implement compare instruction {other:?}"),
        }
    }

    fn shift(&mut self, op: Shift) {
        match op {
            Shift::RolA => {
                let wide = self.snes.cpu.flags.is_acc_16bits();
                let result = self.rol(self.snes.cpu.acc, wide);
                self.set_acc(result, wide);
            }
            Shift::RolAbs { addr } => {
                let ea = addressing::resolve(self.snes, AddrMode::Abs(addr));
                let wide = self.snes.cpu.flags.is_acc_16bits();
                let value = self.read(ea);
                let result = self.rol(value, wide);
                self.write(ea, result);
            }
            other => todo!("Implement shift instruction {other:?}"),
        }
    }

    fn compare_values(&mut self, reg: u16, value: u16, wide: bool) {
        let (mask, sign) = if wide {
            (0xffff, 0x8000)
        } else {
            (0x00ff, 0x0080)
        };
        let reg = reg & mask;
        let value = value & mask;
        let result = reg.wrapping_sub(value) & mask;

        let flags = &mut self.snes.cpu.flags;
        flags.set_carry(reg >= value);
        flags.set_zero(result == 0);
        flags.set_negative(result & sign != 0);
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

    fn xba(&mut self) {
        self.snes.cpu.acc = self.snes.cpu.acc.rotate_left(8);
        self.set_nz(self.snes.cpu.acc, false);
    }

    fn set_nz(&mut self, value: u16, wide: bool) {
        let (mask, sign) = if wide {
            (0xffff, 0x8000)
        } else {
            (0x00ff, 0x0080)
        };

        let flags = &mut self.snes.cpu.flags;
        flags.set_zero(value & mask == 0);
        flags.set_negative(value & sign != 0);
    }

    fn rol(&mut self, value: u16, wide: bool) -> u16 {
        let (msb, mask) = if wide {
            (0x8000, 0xffff)
        } else {
            (0x80, 0x00ff)
        };
        let value = value & mask;
        let carry_in = self.snes.cpu.flags.carry() as u16;
        let result = ((value << 1) | carry_in) & mask;

        self.snes.cpu.flags.set_carry(value & msb != 0);
        self.set_nz(result, wide);

        result
    }
}
