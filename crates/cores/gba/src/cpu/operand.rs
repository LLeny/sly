use super::Cpu;
use super::decoder::arm;
use super::shift::Shift;

impl Cpu {
    pub(super) fn operand_register(&self, index: u8, form: arm::Form) -> u32 {
        match (index, form) {
            // Stored PC is already instruction address + 8 but register shifts read + 12.
            (15, arm::Form::RegReg) => self.pc().wrapping_add(4),
            _ => self.registers[index as usize],
        }
    }

    pub(super) fn read_arm_operand(&self, inst: arm::Instruction) -> (u32, bool) {
        let form = inst.form();
        let (value, shift) = match form {
            arm::Form::Imm | arm::Form::PsrImm => (
                u32::from(inst.imm8()),
                Shift::Ror(u32::from(inst.rotate()) * 2),
            ),
            arm::Form::RegImm | arm::Form::MemReg => {
                let shift = match Shift::decode(inst.shift(), u32::from(inst.amount())) {
                    Shift::Ror(0) => Shift::Rrx,
                    shift => shift,
                };
                (self.registers[inst.rm() as usize], shift)
            }
            arm::Form::RegReg => {
                let amount = self.registers[inst.rs() as usize] & 0xff;
                (
                    self.operand_register(inst.rm(), form),
                    Shift::decode(inst.shift(), amount),
                )
            }
            arm::Form::MemImm => (u32::from(inst.off12()), Shift::Lsl(0)),
            arm::Form::HalfImm => (u32::from(inst.half_offset()), Shift::Lsl(0)),
            arm::Form::HalfReg | arm::Form::PsrReg => {
                (self.registers[inst.rm() as usize], Shift::Lsl(0))
            }
            form => unreachable!("unsupported ARM operand form: {form:?}"),
        };
        shift.apply(value, self.cpsr.c())
    }
}
