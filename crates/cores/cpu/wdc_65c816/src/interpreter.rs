use crate::cpu::Cpu;
use crate::{Bus, decoder};
use decoder::{DisasmCtx, FlagOpsKind, Instruction, Ops};

pub(crate) struct InterpreterView<'a, B: Bus + ?Sized> {
    cpu: &'a mut Cpu,
    bus: &'a mut B,
}

impl<'a, B: Bus + ?Sized> InterpreterView<'a, B> {
    pub(crate) fn new(cpu: &'a mut Cpu, bus: &'a mut B) -> Self {
        Self { cpu, bus }
    }
}

impl<B: Bus + ?Sized> DisasmCtx for InterpreterView<'_, B> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.bus.read(addr as u32)
    }
}

macro_rules! unimplemented_ops {
    ($($name:ident),+ $(,)?) => {
        $(
            #[inline]
            fn $name(&mut self, _inst: Instruction) {}
        )+
    };
}

impl<B: Bus + ?Sized> Ops for InterpreterView<'_, B> {
    fn nop(&mut self, _: Instruction) {}

    fn flag_ops(&mut self, op: FlagOpsKind, _instr: Instruction) {
        match op {
            FlagOpsKind::Clc => self.cpu.flags.set_carry(false),
            FlagOpsKind::Sec => self.cpu.flags.set_carry(true),
            FlagOpsKind::Cli => self.cpu.flags.set_irq_disable(false),
            FlagOpsKind::Sei => self.cpu.flags.set_irq_disable(true),
            FlagOpsKind::Cld => self.cpu.flags.set_decimal(false),
            FlagOpsKind::Sed => self.cpu.flags.set_decimal(true),
            FlagOpsKind::Clv => self.cpu.flags.set_overflow(false),
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
        jml_ial, jmp_abl, jmp_abs, jmp_iax, jmp_ind, jsl_abl, jsr_abs, jsr_iax,
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
        sta_abl, sta_abs, sta_abx, sta_aby, sta_alx, sta_dp, sta_dpx, sta_idl, sta_idly,
        sta_idp, sta_idx, sta_idy, sta_isy, sta_sr,
        stp, stx_abs, stx_dp, stx_dpy, sty_abs, sty_dp, sty_dpx, stz_abs, stz_abx, stz_dp, stz_dpx,
        tax, tay, tcd, tcs, tdc, trb_abs, trb_dp, tsb_abs, tsb_dp, tsc, tsx, txa, txs, txy, tya, tyx,
        wai, wdm_imm, xba, xce,
    }
}
