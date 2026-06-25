use super::decoder;
use crate::Snes;
use decoder::{BranchesKind, DisasmCtx, FlagOpsKind, Instruction, Ops, StoreKind};

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

    /// CIA (current instruction address): the full 24 bit address of the
    /// instruction being executed.
    fn code_addr(&self) -> u64 {
        let cpu = &self.snes.cpu;
        ((cpu.program_bank as u64) << 16) | cpu.pc as u64
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
}

impl DisasmCtx for InterpreterView<'_> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.snes.read(addr as u32)
    }
}

macro_rules! unimplemented_ops {
    ($($name:ident),+ $(,)?) => {
        $(
            #[inline]
            fn $name(&mut self, _inst: Instruction) {
                todo!("Implement instruction {}", stringify!($name));
            }
        )+
    };
}

impl Ops for InterpreterView<'_> {
    fn nop(&mut self, _: Instruction) {}

    fn flag_ops(&mut self, op: FlagOpsKind, _instr: Instruction) {
        let flags = &mut self.snes.cpu.flags;
        match op {
            FlagOpsKind::Clc => flags.set_carry(false),
            FlagOpsKind::Sec => flags.set_carry(true),
            FlagOpsKind::Cli => flags.set_irq_disable(false),
            FlagOpsKind::Sei => flags.set_irq_disable(true),
            FlagOpsKind::Cld => flags.set_decimal(false),
            FlagOpsKind::Sed => flags.set_decimal(true),
            FlagOpsKind::Clv => flags.set_overflow(false),
        }
    }

    fn branches(&mut self, op: BranchesKind, _instr: Instruction) {
        let flags = self.snes.cpu.flags;
        let taken = match op {
            BranchesKind::BraRel => true,
            BranchesKind::BeqRel => flags.zero_(),
            BranchesKind::BneRel => !flags.zero_(),
            BranchesKind::BcsRel => flags.carry(),
            BranchesKind::BccRel => !flags.carry(),
            BranchesKind::BmiRel => flags.negative(),
            BranchesKind::BplRel => !flags.negative(),
            BranchesKind::BvsRel => flags.overflow(),
            BranchesKind::BvcRel => !flags.overflow(),
        };

        if taken {
            let offset = self.read_u8(self.code_addr() + 1) as i8;
            self.branch(offset as i16);
        }
    }

    fn jmp_abs(&mut self, inst: Instruction) {
        let pc = self.code_addr();
        let target = inst.jmp_abs__addr(pc, self) as u16;
        self.jump(target);
    }

    fn jmp_abl(&mut self, inst: Instruction) {
        let pc = self.code_addr();
        let target = inst.jmp_abl__addr(pc, self);
        self.jump_long((target >> 16) as u8, target as u16);
    }

    fn store(&mut self, op: StoreKind, inst: Instruction) {
        match op {
            StoreKind::StzAbs => {
                let data_bank = self.snes.cpu.data_bank;
                let ea = ((data_bank as u32) << 16) | inst.stz_abs__addr(self.code_addr(), self);
                self.snes.write(ea, 0);

                if self.snes.cpu.flags.is_acc_16bits() {
                    self.snes.write(ea + 1, 0);
                }
            }
            _ => todo!("Implement store instruction {op:?}"),
        }
    }

    unimplemented_ops! {
        rep_imm, sep_imm, adc_abl, adc_abs, adc_abx, adc_aby, adc_alx, adc_dp, adc_dpx, adc_idl, adc_idly,
        adc_idp, adc_idx, adc_idy, adc_imm_m16, adc_imm_m8, adc_isy, adc_sr,
        and_abl, and_abs, and_abx, and_aby, and_alx, and_dp, and_dpx, and_idl, and_idly,
        and_idp, and_idx, and_idy, and_imm_m16, and_imm_m8, and_isy, and_sr,
        asl_a, asl_abs, asl_abx, asl_dp, asl_dpx,
        bit_abs, bit_abx, bit_dp, bit_dpx, bit_imm_m16, bit_imm_m8,
        brk_imm, brl_rell,
        cmp_abl, cmp_abs, cmp_abx, cmp_aby, cmp_alx, cmp_dp, cmp_dpx, cmp_idl, cmp_idly,
        cmp_idp, cmp_idx, cmp_idy, cmp_imm_m16, cmp_imm_m8, cmp_isy, cmp_sr,
        cop_imm, cpx_abs, cpx_dp, cpx_imm_x16, cpx_imm_x8, cpy_abs, cpy_dp, cpy_imm_x16, cpy_imm_x8,
        dec_abs, dec_abx, dec_dp, dec_dpx,
        eor_abl, eor_abs, eor_abx, eor_aby, eor_alx, eor_dp, eor_dpx, eor_idl, eor_idly,
        eor_idp, eor_idx, eor_idy, eor_imm_m16, eor_imm_m8, eor_isy, eor_sr,
        inc_abs, inc_abx, inc_dp, inc_dpx,
        jml_ial, jmp_iax, jmp_ind, jsl_abl, jsr_abs, jsr_iax,
        lda_abl, lda_abs, lda_abx, lda_aby, lda_alx, lda_dp, lda_dpx, lda_idl, lda_idly,
        lda_idp, lda_idx, lda_idy, lda_imm_m16, lda_imm_m8, lda_isy, lda_sr,
        ldx_abs, ldx_aby, ldx_dp, ldx_dpy, ldx_imm_x16, ldx_imm_x8,
        ldy_abs, ldy_abx, ldy_dp, ldy_dpx, ldy_imm_x16, ldy_imm_x8,
        lsr_a, lsr_abs, lsr_abx, lsr_dp, lsr_dpx,
        mvn_bm, mvp_bm,
        ora_abl, ora_abs, ora_abx, ora_aby, ora_alx, ora_dp, ora_dpx, ora_idl, ora_idly,
        ora_idp, ora_idx, ora_idy, ora_imm_m16, ora_imm_m8, ora_isy, ora_sr,
        pea_abs, pei_idp, per_rell, pha, phb, phd, phk, php, phx, phy,
        pla, plb, pld, plp, plx, ply,
        rol_a, rol_abs, rol_abx, rol_dp, rol_dpx, ror_a, ror_abs, ror_abx, ror_dp, ror_dpx,
        rti, rtl, rts,
        sbc_abl, sbc_abs, sbc_abx, sbc_aby, sbc_alx, sbc_dp, sbc_dpx, sbc_idl, sbc_idly,
        sbc_idp, sbc_idx, sbc_idy, sbc_imm_m16, sbc_imm_m8, sbc_isy, sbc_sr,
        stp,
        tax, tay, tcd, tcs, tdc, trb_abs, trb_dp, tsb_abs, tsb_dp, tsc, tsx, txa, txs, txy, tya, tyx,
        wai, wdm_imm, xba, xce,
    }
}
