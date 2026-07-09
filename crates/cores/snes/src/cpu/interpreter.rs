use super::addressing::resolve;
use super::decoder::{Form, Mnemonic};
use super::operand::Arg;
use crate::Snes;
use crate::alu::{AluResult, ShiftOp, Width, adc_bcd8, add_binary, bcd16, sbc_bcd8};
#[cfg(feature = "debug")]
use crate::callstack::{CallFrame, CallKind};
use crate::cpu::{Cpu, Flags};
use crate::mmio::{Address, MemoryMap};

#[derive(Clone, Copy)]
enum StackWrap {
    Confined,
    Wide,
}

impl StackWrap {
    fn step(self, cpu: &Cpu, sp: u16) -> u16 {
        match self {
            StackWrap::Confined => cpu.confine_sp(sp),
            StackWrap::Wide => sp,
        }
    }
}

trait StackWord: Copy {
    fn to_stack_bytes(self) -> impl Iterator<Item = u8>;
    fn from_stack_bytes(pop: impl FnMut() -> u8) -> Self;
}

impl StackWord for u8 {
    fn to_stack_bytes(self) -> impl Iterator<Item = u8> {
        std::iter::once(self)
    }

    fn from_stack_bytes(mut pop: impl FnMut() -> u8) -> Self {
        pop()
    }
}

impl StackWord for u16 {
    fn to_stack_bytes(self) -> impl Iterator<Item = u8> {
        [(self >> 8) as u8, self as u8].into_iter()
    }

    fn from_stack_bytes(mut pop: impl FnMut() -> u8) -> Self {
        let lo = pop() as u16;
        let hi = pop() as u16;
        lo | (hi << 8)
    }
}

pub(crate) struct Interpreter<'a, M: MemoryMap> {
    snes: &'a mut Snes<M>,
    next_bank: u8,
    next_pc: u16,
}

impl<'a, M: MemoryMap> Interpreter<'a, M> {
    pub(crate) fn new(snes: &'a mut Snes<M>, len: u8) -> Self {
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
    fn next(&self) -> (u8, u16) {
        (self.next_bank, self.next_pc)
    }

    pub(crate) fn commit(self) {
        self.snes.cpu.program_bank = self.next_bank;
        self.snes.cpu.pc = self.next_pc;
        self.snes.cpu.normalize_emulation();
        self.snes.cpu.narrow_index();
    }

    /// Le juice.
    pub(crate) fn exec(&mut self, mnemonic: Mnemonic, form: Form, arg: Arg) {
        use Form as F;
        use Mnemonic as M;

        let aw = self.snes.cpu.acc_wide();
        let iw = self.snes.cpu.index_wide();
        let imm = |arg: Arg| match arg {
            Arg::Imm(v) => v,
            _ => 0,
        };

        match mnemonic {
            M::Invalid | M::Nop | M::WdmImm | M::Stp | M::Wai => {}

            // loads and stores
            M::Lda => {
                let v = self.operand(form, arg, aw);
                self.lda(v);
            }
            M::Ldx => {
                let v = self.operand(form, arg, iw);
                self.ldx(v);
            }
            M::Ldy => {
                let v = self.operand(form, arg, iw);
                self.ldy(v);
            }
            M::Sta => self.store(form, arg, self.snes.cpu.acc, aw),
            M::Stx => self.store(form, arg, self.snes.cpu.index_x, iw),
            M::Sty => self.store(form, arg, self.snes.cpu.index_y, iw),
            M::Stz => self.store(form, arg, 0, aw),

            // accumulator arithmetic and logic
            M::Adc => {
                let v = self.operand(form, arg, aw);
                self.adc(v);
            }
            M::Sbc => {
                let v = self.operand(form, arg, aw);
                self.sbc(v);
            }
            M::And => {
                let v = self.operand(form, arg, aw);
                self.lda(self.snes.cpu.acc & v);
            }
            M::Ora => {
                let v = self.operand(form, arg, aw);
                self.lda(self.snes.cpu.acc | v);
            }
            M::Eor => {
                let v = self.operand(form, arg, aw);
                self.lda(self.snes.cpu.acc ^ v);
            }
            M::Cmp => {
                let v = self.operand(form, arg, aw);
                self.compare_values(self.snes.cpu.acc, v, aw);
            }
            M::Cpx => {
                let v = self.operand(form, arg, iw);
                self.compare_values(self.snes.cpu.index_x, v, iw);
            }
            M::Cpy => {
                let v = self.operand(form, arg, iw);
                self.compare_values(self.snes.cpu.index_y, v, iw);
            }
            M::Bit => match form {
                F::Imm => self.bit_imm(imm(arg)),
                _ => self.bit(form, arg),
            },

            // alu shit
            M::Asl => self.shift(ShiftOp::Asl, form, arg),
            M::Lsr => self.shift(ShiftOp::Lsr, form, arg),
            M::Rol => self.shift(ShiftOp::Rol, form, arg),
            M::Ror => self.shift(ShiftOp::Ror, form, arg),
            M::Inc => match form {
                F::A => self.lda(self.snes.cpu.acc.wrapping_add(1)),
                _ => self.mem_step(form, arg, 1),
            },
            M::Dec => match form {
                F::A => self.lda(self.snes.cpu.acc.wrapping_sub(1)),
                _ => self.mem_step(form, arg, u16::MAX),
            },
            M::Inx => self.ldx(self.snes.cpu.index_x.wrapping_add(1)),
            M::Dex => self.ldx(self.snes.cpu.index_x.wrapping_sub(1)),
            M::Iny => self.ldy(self.snes.cpu.index_y.wrapping_add(1)),
            M::Dey => self.ldy(self.snes.cpu.index_y.wrapping_sub(1)),
            M::Tsb => self.bit_modify(form, arg, true),
            M::Trb => self.bit_modify(form, arg, false),

            // branches
            M::Bra
            | M::Brl
            | M::Beq
            | M::Bne
            | M::Bcs
            | M::Bcc
            | M::Bmi
            | M::Bpl
            | M::Bvs
            | M::Bvc => {
                let taken = crate::cpu::branch_taken(&self.snes.cpu.flags, mnemonic);
                self.branch(taken, arg);
            }

            // jumps, calls and returns
            M::Jmp => match (form, arg) {
                (F::Abs, Arg::Abs(a)) => self.set_pc(a),
                (F::Abl, Arg::Long(a)) => self.set_pc_addr(Address::from_24bit(a)),
                (F::Ind, Arg::Abs(a)) => {
                    let target = self.snes.read16(Address::new(0, a));
                    self.set_pc(target);
                }
                (F::Iax, Arg::Abs(a)) => {
                    let target = self.indexed_indirect(a);
                    self.set_pc(target);
                }
                _ => unreachable!("jmp form"),
            },
            M::Jml => {
                let target = self.snes.read24(Address::new(0, arg.abs()));
                self.set_pc_addr(Address::from_24bit(target));
            }
            M::Jsr => {
                self.push(self.next_pc.wrapping_sub(1), StackWrap::Confined);
                match (form, arg) {
                    (F::Abs, Arg::Abs(a)) => self.set_pc(a),
                    (F::Iax, Arg::Abs(a)) => {
                        let target = self.indexed_indirect(a);
                        self.set_pc(target);
                    }
                    _ => unreachable!("jsr form"),
                }

                #[cfg(feature = "debug")]
                self.enter_call(
                    CallKind::Call,
                    Address::new(self.snes.cpu.program_bank, self.snes.cpu.pc).as_24bit(),
                    Address::new(self.next_bank, self.next_pc).as_24bit(),
                );
            }
            M::Jsl => {
                self.push(self.next_bank, StackWrap::Wide);
                self.push(self.next_pc.wrapping_sub(1), StackWrap::Wide);
                match arg {
                    Arg::Long(a) => self.set_pc_addr(Address::from_24bit(a)),
                    _ => unreachable!("jsl form"),
                }

                #[cfg(feature = "debug")]
                self.enter_call(
                    CallKind::LongCall,
                    Address::new(self.snes.cpu.program_bank, self.snes.cpu.pc).as_24bit(),
                    Address::new(self.next_bank, self.next_pc).as_24bit(),
                );
            }
            M::Rts => {
                let return_pc = self.pop::<u16>(StackWrap::Confined);
                self.set_pc(return_pc.wrapping_add(1));

                #[cfg(feature = "debug")]
                self.leave_call();
            }
            M::Rtl => {
                let return_pc = self.pop::<u16>(StackWrap::Wide);
                let bank = self.pop::<u8>(StackWrap::Wide);
                self.set_pc_long(bank, return_pc.wrapping_add(1));

                #[cfg(feature = "debug")]
                self.leave_call();
            }
            M::Rti => {
                let flags = self.pop::<u8>(StackWrap::Confined);
                self.snes.cpu.flags = Flags::from_raw(flags);
                let return_pc = self.pop::<u16>(StackWrap::Confined);
                if self.snes.cpu.emulation {
                    self.set_pc(return_pc);
                } else {
                    let bank = self.pop::<u8>(StackWrap::Confined);
                    self.set_pc_long(bank, return_pc);
                }

                #[cfg(feature = "debug")]
                self.leave_call();
            }

            // stack
            M::Pha => self.push_reg(self.snes.cpu.acc, aw),
            M::Phx => self.push_reg(self.snes.cpu.index_x, iw),
            M::Phy => self.push_reg(self.snes.cpu.index_y, iw),
            M::Php => self.push(self.snes.cpu.flags.raw(), StackWrap::Confined),
            M::Phb => self.push(self.snes.cpu.data_bank, StackWrap::Wide),
            M::Phk => self.push(self.snes.cpu.program_bank, StackWrap::Wide),
            M::Phd => self.push(self.snes.cpu.direct_page, StackWrap::Wide),
            M::Pea => self.push(arg.abs(), StackWrap::Wide),
            M::Pei => {
                let value = self.operand_addr(form, arg).0.address();
                self.push(value, StackWrap::Wide);
            }
            M::Per => {
                let Arg::Rel(disp) = arg else {
                    unreachable!("per form")
                };
                let target = self.next_pc.wrapping_add(disp as u16);
                self.push(target, StackWrap::Wide);
            }
            M::Pla => {
                let value = self.pop_reg(aw);
                self.lda(value);
            }
            M::Plx => {
                let value = self.pop_reg(iw);
                self.ldx(value);
            }
            M::Ply => {
                let value = self.pop_reg(iw);
                self.ldy(value);
            }
            M::Plb => {
                self.snes.cpu.data_bank = self.pop::<u8>(StackWrap::Wide);
                self.set_nz(self.snes.cpu.data_bank as u16, false);
            }
            M::Pld => {
                self.snes.cpu.direct_page = self.pop::<u16>(StackWrap::Wide);
                self.set_nz(self.snes.cpu.direct_page, true);
            }
            M::Plp => self.snes.cpu.flags = Flags::from_raw(self.pop::<u8>(StackWrap::Confined)),

            // transfers
            M::Tax => self.ldx(self.snes.cpu.acc),
            M::Tay => self.ldy(self.snes.cpu.acc),
            M::Tsx => self.ldx(self.snes.cpu.stack_pointer),
            M::Txa => self.lda(self.snes.cpu.index_x),
            M::Tya => self.lda(self.snes.cpu.index_y),
            M::Txy => self.ldy(self.snes.cpu.index_x),
            M::Tyx => self.ldx(self.snes.cpu.index_y),
            M::Txs => self.set_sp(self.snes.cpu.index_x),
            M::Tcs => self.set_sp(self.snes.cpu.acc),
            M::Tcd => {
                self.snes.cpu.direct_page = self.snes.cpu.acc;
                self.set_nz(self.snes.cpu.direct_page, true);
            }
            M::Tdc => {
                self.snes.cpu.acc = self.snes.cpu.direct_page;
                self.set_nz(self.snes.cpu.acc, true);
            }
            M::Tsc => {
                self.snes.cpu.acc = self.snes.cpu.stack_pointer;
                self.set_nz(self.snes.cpu.acc, true);
            }

            // flags
            M::Clc => self.snes.cpu.flags.set_carry(false),
            M::Sec => self.snes.cpu.flags.set_carry(true),
            M::Cli => self.snes.cpu.flags.set_irq_disable(false),
            M::Sei => self.snes.cpu.flags.set_irq_disable(true),
            M::Cld => self.snes.cpu.flags.set_decimal(false),
            M::Sed => self.snes.cpu.flags.set_decimal(true),
            M::Clv => self.snes.cpu.flags.set_overflow(false),
            M::RepImm => {
                let raw = self.snes.cpu.flags.raw();
                self.snes.cpu.flags = Flags::from_raw(raw & !(imm(arg) as u8));
            }
            M::SepImm => {
                let raw = self.snes.cpu.flags.raw();
                self.snes.cpu.flags = Flags::from_raw(raw | (imm(arg) as u8));
            }

            // mode switches
            M::Xce => {
                let cpu = &mut self.snes.cpu;
                let carry = cpu.flags.carry();
                cpu.flags.set_carry(cpu.emulation);
                cpu.emulation = carry;
            }
            M::Xba => {
                self.snes.cpu.acc = self.snes.cpu.acc.rotate_left(8);
                self.set_nz(self.snes.cpu.acc, false);
            }

            // block moves
            M::Mvn => self.block_move(arg, false),
            M::Mvp => self.block_move(arg, true),

            // software interrupts
            M::BrkImm => self.service_interrupt(0xFFE6, 0xFFFE),
            M::CopImm => self.service_interrupt(0xFFE4, 0xFFF4),
        }
    }

    fn set_acc(&mut self, value: u16, wide: bool) {
        self.snes.cpu.acc = if wide {
            value
        } else {
            (self.snes.cpu.acc & 0xff00) | (value & 0x00ff)
        };
    }

    fn lda(&mut self, value: u16) {
        let wide = self.snes.cpu.acc_wide();

        self.set_acc(value, wide);
        self.set_nz(value, wide);
    }

    fn ldx(&mut self, value: u16) {
        let wide = self.snes.cpu.index_wide();

        self.snes.cpu.index_x = value & wide.mask();
        self.set_nz(self.snes.cpu.index_x, wide);
    }

    fn ldy(&mut self, value: u16) {
        let wide = self.snes.cpu.index_wide();

        self.snes.cpu.index_y = value & wide.mask();
        self.set_nz(self.snes.cpu.index_y, wide);
    }

    fn adc(&mut self, value: u16) {
        self.add_sub(adc_bcd8, value, value);
    }

    fn sbc(&mut self, value: u16) {
        let wide = self.snes.cpu.acc_wide();
        self.add_sub(sbc_bcd8, value, value ^ wide.mask());
    }

    fn add_sub(&mut self, bcd8: fn(u16, u16, bool) -> AluResult, value: u16, binary: u16) {
        let wide = self.snes.cpu.acc_wide();
        let a = self.snes.cpu.acc;
        let carry = self.snes.cpu.flags.carry();

        let flags = if self.snes.cpu.flags.decimal() {
            if wide {
                bcd16(bcd8, a, value, carry)
            } else {
                bcd8(a, value, carry)
            }
        } else {
            add_binary(a, binary, carry, wide)
        };

        self.apply_alu_result(flags, wide);
    }

    fn apply_alu_result(&mut self, alu: AluResult, wide: bool) {
        let flags = &mut self.snes.cpu.flags;
        flags.set_carry(alu.carry);
        flags.set_overflow(alu.overflow);
        flags.set_negative(alu.negative);
        flags.set_zero(alu.zero);

        self.set_acc(alu.result, wide);
    }

    fn compare_values(&mut self, reg: u16, value: u16, wide: bool) {
        let mask = wide.mask();
        let sign = wide.sign_bit();

        let reg = reg & mask;
        let value = value & mask;
        let result = reg.wrapping_sub(value) & mask;

        let flags = &mut self.snes.cpu.flags;
        flags.set_carry(reg >= value);
        flags.set_zero(result == 0);
        flags.set_negative(result & sign != 0);
    }

    fn bit(&mut self, form: Form, arg: Arg) {
        let wide = self.snes.cpu.acc_wide();
        let value = self.operand(form, arg, wide);
        let mask = wide.mask();
        let sign = wide.sign_bit();
        let overflow_bit = sign >> 1;

        let zero = (self.snes.cpu.acc & value) & mask == 0;
        let flags = &mut self.snes.cpu.flags;
        flags.set_zero(zero);
        flags.set_negative(value & sign != 0);
        flags.set_overflow(value & overflow_bit != 0);
    }

    fn bit_imm(&mut self, value: u16) {
        let mask = self.snes.cpu.acc_wide().mask();
        let zero = (self.snes.cpu.acc & value) & mask == 0;
        self.snes.cpu.flags.set_zero(zero);
    }

    fn shift(&mut self, op: ShiftOp, form: Form, arg: Arg) {
        let wide = self.snes.cpu.acc_wide();
        if matches!(form, Form::A) {
            let result = self.do_shift(op, self.snes.cpu.acc, wide);
            self.set_acc(result, wide);
        } else {
            let (lo, hi) = self.operand_addr(form, arg);
            let value = self.read_value(lo, hi, wide);
            let result = self.do_shift(op, value, wide);
            self.write_value(lo, hi, result, wide);
        }
    }

    fn do_shift(&mut self, op: ShiftOp, value: u16, wide: bool) -> u16 {
        let carry_in = self.snes.cpu.flags.carry();
        let (result, carry_out) = crate::alu::shift(op, value, carry_in, wide);

        self.snes.cpu.flags.set_carry(carry_out);
        self.set_nz(result, wide);
        result
    }

    fn mem_step(&mut self, form: Form, arg: Arg, delta: u16) {
        let wide = self.snes.cpu.acc_wide();
        let (lo, hi) = self.operand_addr(form, arg);
        let value = self.read_value(lo, hi, wide);

        let result = value.wrapping_add(delta) & wide.mask();

        self.write_value(lo, hi, result, wide);
        self.set_nz(result, wide);
    }

    fn bit_modify(&mut self, form: Form, arg: Arg, set: bool) {
        let wide = self.snes.cpu.acc_wide();
        let (lo, hi) = self.operand_addr(form, arg);
        let value = self.read_value(lo, hi, wide);
        let acc = self.snes.cpu.acc;
        let mask = wide.mask();

        self.snes.cpu.flags.set_zero((value & acc) & mask == 0);
        let result = if set { value | acc } else { value & !acc } & mask;
        self.write_value(lo, hi, result, wide);
    }

    fn branch(&mut self, taken: bool, arg: Arg) {
        let Arg::Rel(disp) = arg else {
            unreachable!("branch form")
        };
        if taken {
            self.next_pc = self.next_pc.wrapping_add(disp as u16);
        }
    }

    fn block_move(&mut self, arg: Arg, descending: bool) {
        let Arg::Move { dst, src } = arg else {
            unreachable!("block move form")
        };

        let byte = self.snes.read8(Address::new(src, self.snes.cpu.index_x));
        self.snes
            .write8(Address::new(dst, self.snes.cpu.index_y), byte);
        self.snes.cpu.data_bank = dst;

        let step = if descending { u16::MAX } else { 1 };
        self.snes.cpu.index_x = self.snes.cpu.index_x.wrapping_add(step);
        self.snes.cpu.index_y = self.snes.cpu.index_y.wrapping_add(step);
        self.snes.cpu.acc = self.snes.cpu.acc.wrapping_sub(1);

        if self.snes.cpu.acc != 0xffff {
            self.set_pc(self.snes.cpu.pc);
        }
    }

    fn operand_addr(&self, form: Form, arg: Arg) -> (Address, Address) {
        resolve(self.snes, form, arg).expect("form without a memory operand")
    }

    fn read_value(&mut self, lo: Address, hi: Address, wide: bool) -> u16 {
        let low = self.snes.read8(lo) as u16;
        if wide {
            low | ((self.snes.read8(hi) as u16) << 8)
        } else {
            low
        }
    }

    fn write_value(&mut self, lo: Address, hi: Address, value: u16, wide: bool) {
        self.snes.write8(lo, value as u8);
        if wide {
            self.snes.write8(hi, (value >> 8) as u8);
        }
    }

    fn operand(&mut self, form: Form, arg: Arg, wide: bool) -> u16 {
        match arg {
            Arg::Imm(v) => v,
            _ => {
                let (lo, hi) = self.operand_addr(form, arg);
                self.read_value(lo, hi, wide)
            }
        }
    }

    fn store(&mut self, form: Form, arg: Arg, value: u16, wide: bool) {
        let (lo, hi) = self.operand_addr(form, arg);
        self.write_value(lo, hi, value, wide);
    }

    fn set_nz(&mut self, value: u16, wide: bool) {
        let flags = &mut self.snes.cpu.flags;
        flags.set_zero(value & wide.mask() == 0);
        flags.set_negative(value & wide.sign_bit() != 0);
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

    /// Redirect execution to a 24bit address.
    fn set_pc_addr(&mut self, target: Address) {
        self.set_pc_long(target.bank(), target.address());
    }

    /// Resolve the (abs,x) indirect target within the program bank.
    fn indexed_indirect(&mut self, addr: u16) -> u16 {
        let bank = self.snes.cpu.program_bank;
        let ptr = addr.wrapping_add(self.snes.cpu.index_x);
        self.snes.read16(Address::new(bank, ptr))
    }

    fn set_sp(&mut self, value: u16) {
        self.snes.cpu.stack_pointer = self.snes.cpu.confine_sp(value);
    }

    /// Record a call frame opened by the current instruction.
    #[cfg(feature = "debug")]
    fn enter_call(&mut self, kind: CallKind, caller: u32, target: u32) {
        crate::callstack::push_frame(
            &mut self.snes.cpu.call_stack,
            CallFrame {
                kind,
                caller,
                target,
                sp: self.snes.cpu.stack_pointer,
            },
        );
    }

    /// Unwind call frames the stack pointer has moved past on return.
    #[cfg(feature = "debug")]
    fn leave_call(&mut self) {
        crate::callstack::unwind(&mut self.snes.cpu.call_stack, self.snes.cpu.stack_pointer);
    }

    /// Push a `u8` or `u16` onto the stack, high byte first.
    fn push<T: StackWord>(&mut self, value: T, wrap: StackWrap) {
        for byte in value.to_stack_bytes() {
            let sp = self.snes.cpu.stack_pointer;
            self.snes.write8(Address::new(0, sp), byte);
            self.snes.cpu.stack_pointer = wrap.step(&self.snes.cpu, sp.wrapping_sub(1));
        }
    }

    /// Pop a `u8` or `u16` off the stack, low byte first (the `push` inverse).
    fn pop<T: StackWord>(&mut self, wrap: StackWrap) -> T {
        T::from_stack_bytes(|| {
            let sp = wrap.step(&self.snes.cpu, self.snes.cpu.stack_pointer.wrapping_add(1));
            self.snes.cpu.stack_pointer = sp;
            self.snes.read8(Address::new(0, sp))
        })
    }

    fn push_reg(&mut self, value: u16, wide: bool) {
        if wide {
            self.push(value, StackWrap::Confined);
        } else {
            self.push(value as u8, StackWrap::Confined);
        }
    }

    fn pop_reg(&mut self, wide: bool) -> u16 {
        if wide {
            self.pop::<u16>(StackWrap::Confined)
        } else {
            self.pop::<u8>(StackWrap::Confined) as u16
        }
    }

    pub(crate) fn service_interrupt(&mut self, native_vector: u16, emu_vector: u16) {
        let (program_bank, return_pc) = self.next();
        let emulation = self.snes.cpu.emulation;

        if !emulation {
            self.push(program_bank, StackWrap::Confined);
        }

        self.push(return_pc, StackWrap::Confined);
        self.push(self.snes.cpu.flags.raw(), StackWrap::Confined);

        self.snes.cpu.flags.set_irq_disable(true);
        self.snes.cpu.flags.set_decimal(false);
        self.snes.cpu.program_bank = 0;

        let vector = if emulation { emu_vector } else { native_vector };
        let target = self.snes.read16(Address::new(0, vector));
        self.set_pc_long(0, target);

        #[cfg(feature = "debug")]
        self.enter_call(
            CallKind::Interrupt,
            Address::new(program_bank, return_pc).as_24bit(),
            Address::new(0, target).as_24bit(),
        );
    }
}
