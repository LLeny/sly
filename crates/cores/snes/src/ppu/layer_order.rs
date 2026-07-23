use crate::ppu::regs::{BackgroundMode, Priority};

#[derive(Clone, Copy)]
pub(super) enum RenderPass {
    Background { layer: usize, priority: Priority },
    Objects { priority: u8 },
}

const fn bg(layer: usize, priority: Priority) -> RenderPass {
    RenderPass::Background { layer, priority }
}

const fn obj(priority: u8) -> RenderPass {
    RenderPass::Objects { priority }
}

use Priority::{High, Normal};

const MODE0: &[RenderPass] = &[
    obj(3),
    bg(0, High),
    bg(1, High),
    obj(2),
    bg(0, Normal),
    bg(1, Normal),
    obj(1),
    bg(2, High),
    bg(3, High),
    obj(0),
    bg(2, Normal),
    bg(3, Normal),
];

const MODE1_BG3_FRONT: &[RenderPass] = &[
    bg(2, High),
    obj(3),
    bg(0, High),
    bg(1, High),
    obj(2),
    bg(0, Normal),
    bg(1, Normal),
    obj(1),
    obj(0),
    bg(2, Normal),
];

const MODE1: &[RenderPass] = &[
    obj(3),
    bg(0, High),
    bg(1, High),
    obj(2),
    bg(0, Normal),
    bg(1, Normal),
    obj(1),
    bg(2, High),
    obj(0),
    bg(2, Normal),
];

const TWO_BACKGROUNDS: &[RenderPass] = &[
    obj(3),
    bg(0, High),
    obj(2),
    bg(1, High),
    obj(1),
    bg(0, Normal),
    obj(0),
    bg(1, Normal),
];

const MODE7: &[RenderPass] = &[obj(3), obj(2), obj(1), bg(0, Normal), obj(0)];

const MODE7_EXTBG: &[RenderPass] = &[
    obj(3),
    obj(2),
    bg(1, High),
    obj(1),
    bg(0, Normal),
    obj(0),
    bg(1, Normal),
];

pub(super) fn front_to_back_passes(
    mode: BackgroundMode,
    bg3_priority: Priority,
    extbg: bool,
) -> &'static [RenderPass] {
    match (mode, bg3_priority) {
        (BackgroundMode::Mode0, _) => MODE0,
        (BackgroundMode::Mode1, Priority::High) => MODE1_BG3_FRONT,
        (BackgroundMode::Mode1, Priority::Normal) => MODE1,
        (BackgroundMode::Mode7, _) if extbg => MODE7_EXTBG,
        (BackgroundMode::Mode7, _) => MODE7,
        // TODO: define priority orders for modes 2-6.
        _ => TWO_BACKGROUNDS,
    }
}
