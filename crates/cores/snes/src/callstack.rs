#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallKind {
    Call,
    LongCall,
    Interrupt,
}

impl CallKind {
    pub fn tag(self) -> &'static str {
        match self {
            CallKind::Call => "JSR",
            CallKind::LongCall => "JSL",
            CallKind::Interrupt => "IRQ",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CallFrame {
    pub kind: CallKind,
    pub caller: u32,
    pub target: u32,
}
