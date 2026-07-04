use crate::scheduler::{MASTER_CLOCKS_PER_SCANLINE, Scheduler, VBLANK_START_SCANLINE};
use crate::{MemoryMap, Snes};

const HBLANK_START_CLOCK: u64 = 274 * 4;

pub(crate) fn vblank_start<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    snes.irq.set_vblank(true);
}

pub(crate) fn vblank_end<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    snes.irq.set_vblank(false);
}

pub(crate) fn in_vblank<M: MemoryMap>(scheduler: &Scheduler<M>) -> bool {
    scheduler.cycles() % scheduler.cycles_per_frame()
        >= VBLANK_START_SCANLINE * MASTER_CLOCKS_PER_SCANLINE
}

pub(crate) fn in_hblank<M: MemoryMap>(scheduler: &Scheduler<M>) -> bool {
    scheduler.cycles() % MASTER_CLOCKS_PER_SCANLINE >= HBLANK_START_CLOCK
}
