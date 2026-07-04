use crate::{MemoryMap, Snes};

pub(crate) fn vblank_tick<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    println!("VBLANK TICK");
}