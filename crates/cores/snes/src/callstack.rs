#[cfg(feature = "debug")]
const MAX_DEPTH: usize = 512;

#[cfg(feature = "debug")]
pub(crate) fn push_frame(stack: &mut Vec<CallFrame>, frame: CallFrame) {
    unwind(stack, frame.sp);
    if stack.len() >= MAX_DEPTH {
        stack.drain(..MAX_DEPTH / 2);
    }
    stack.push(frame);
}

#[cfg(feature = "debug")]
pub(crate) fn unwind(stack: &mut Vec<CallFrame>, current_sp: u16) {
    while stack.last().is_some_and(|f| f.sp < current_sp) {
        stack.pop();
    }
}

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
    pub sp: u16,
}
