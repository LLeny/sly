use super::*;

pub(super) const EVENT_SCANLINE: &str = "scanline";
pub(super) const EVENT_NMI: &str = "nmi";
pub(super) const EVENT_IRQ: &str = "irq";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct EventBaseline {
    scanline: u32,
    nmi: u64,
    irq: u64,
}

#[derive(Default)]
pub(super) struct EventWatch {
    pub(super) ids: Vec<Arc<str>>,
    scanline: bool,
    nmi: bool,
    irq: bool,
    scanline_id: Arc<str>,
    nmi_id: Arc<str>,
    irq_id: Arc<str>,
    vblank: Arc<str>,
    empty: Arc<str>,
    irq_h: Arc<str>,
    irq_v: Arc<str>,
    irq_hv: Arc<str>,
    irq_none: Arc<str>,
}

impl EventWatch {
    pub(super) fn new() -> Self {
        Self {
            scanline_id: Arc::from(EVENT_SCANLINE),
            nmi_id: Arc::from(EVENT_NMI),
            irq_id: Arc::from(EVENT_IRQ),
            vblank: Arc::from("vblank"),
            empty: Arc::from(""),
            irq_h: Arc::from("H"),
            irq_v: Arc::from("V"),
            irq_hv: Arc::from("HV"),
            irq_none: Arc::from("none"),
            ..Self::default()
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub(super) fn set(&mut self, ids: &[Arc<str>]) {
        self.ids = ids.to_vec();
        self.scanline = self.watches(EVENT_SCANLINE);
        self.nmi = self.watches(EVENT_NMI);
        self.irq = self.watches(EVENT_IRQ);
    }

    pub(super) fn watches(&self, id: &str) -> bool {
        self.ids.iter().any(|watch| &**watch == id)
    }
}

impl SnesDebug {
    pub(super) fn execute_hit(&self, breakpoints: &[BreakpointMark]) -> bool {
        let pcs = [
            (CPU_MAIN, self.core.cpu.pc as u32),
            (CPU_SPC, self.core.apu.spc700.pc as u32),
            (superfx::CPU_GSU, superfx::pc(&self.core.superfx) as u32),
        ];

        breakpoints
            .iter()
            .filter(|m| m.kind == BreakpointKind::Execute)
            .any(|m| {
                pcs.iter().any(|(id, pc)| {
                    m.cpu.as_str() == *id && (m.start & 0xFFFF..=m.end & 0xFFFF).contains(pc)
                })
            })
    }

    pub(super) fn captured(
        &mut self,
        mode: TrackMode,
        run: impl FnOnce(&mut Self),
    ) -> MemoryAccess {
        self.core.mmu.access.begin(mode);
        self.core.apu.access.begin(mode);
        self.core.apu.access.resume();

        run(self);

        self.core.mmu.access.stop();
        self.core.apu.access.stop();

        let mut access = self.core.mmu.access.take();
        access.merge(&self.core.apu.access.take());
        access
    }

    pub(super) fn step_captured(&mut self) -> MemoryAccess {
        self.captured(TrackMode::Full, |debug| debug.core.step())
    }

    pub(super) fn track_mode(&self) -> TrackMode {
        use std::sync::atomic::Ordering::Relaxed;

        for (frames, mode) in [
            (&self.capture_frames, TrackMode::Full),
            (&self.track_frames, TrackMode::LastOnly),
        ] {
            let remaining = frames.load(Relaxed);
            if remaining > 0 {
                frames.store(remaining - 1, Relaxed);
                return mode;
            }
        }

        TrackMode::Off
    }

    pub(super) fn event_outcome(&self, frame_end: u64) -> RunOutcome {
        if self.core.scheduler.cycles() >= frame_end {
            RunOutcome::Done
        } else {
            RunOutcome::Breakpoint
        }
    }

    pub(super) fn scanline(&self) -> u32 {
        self.core
            .scheduler
            .scanline_at(self.core.scheduler.cycles()) as u32
    }

    pub(super) fn event_snapshot(&self) -> EventBaseline {
        EventBaseline {
            scanline: if self.watch.scanline {
                self.scanline()
            } else {
                0
            },
            nmi: if self.watch.nmi {
                self.core.cpu.interrupts.nmi
            } else {
                0
            },
            irq: if self.watch.irq {
                self.core.cpu.interrupts.irq
            } else {
                0
            },
        }
    }

    pub(super) fn irq_detail(&self) -> Arc<str> {
        match self.core.irq.hv_timer_condition() {
            1 => self.watch.irq_h.clone(),
            2 => self.watch.irq_v.clone(),
            3 => self.watch.irq_hv.clone(),
            _ => self.watch.irq_none.clone(),
        }
    }

    pub(super) fn scan_events(&mut self) -> bool {
        if self.watch.is_empty() {
            return false;
        }

        let now = self.event_snapshot();
        if now == self.event_baseline {
            return false;
        }

        let before = std::mem::replace(&mut self.event_baseline, now);
        let pc = self.core.cpu.pc as u32;
        // NMI and IRQ report the line they fired on, so it is worth the
        // division here even when the scanline itself is not watched.
        let line = if self.watch.scanline {
            now.scanline
        } else {
            self.scanline()
        };
        let before_len = self.events.len();

        if self.watch.scanline && now.scanline != before.scanline {
            self.events.push(EventHit {
                id: self.watch.scanline_id.clone(),
                value: line,
                detail: self.watch.empty.clone(),
                pc,
            });
        }
        if self.watch.nmi && now.nmi != before.nmi {
            self.events.push(EventHit {
                id: self.watch.nmi_id.clone(),
                value: line,
                detail: self.watch.vblank.clone(),
                pc,
            });
        }
        if self.watch.irq && now.irq != before.irq {
            self.events.push(EventHit {
                id: self.watch.irq_id.clone(),
                value: line,
                detail: self.irq_detail(),
                pc,
            });
        }

        self.events.len() != before_len
    }

    pub(super) fn collect_hits(
        &self,
        breakpoints: &[BreakpointMark],
        access: &MemoryAccess,
    ) -> Vec<HookHit> {
        const MAX_HITS: usize = 64;

        let pcs = [
            (CPU_MAIN, self.core.cpu.pc as u32),
            (CPU_SPC, self.core.apu.spc700.pc as u32),
            (superfx::CPU_GSU, superfx::pc(&self.core.superfx) as u32),
        ];
        let main_pc = self.core.cpu.pc as u32;

        let mut hits = Vec::new();
        'marks: for mark in breakpoints {
            if hits.len() >= MAX_HITS {
                break;
            }

            match mark.kind {
                BreakpointKind::Execute => {
                    let hit = pcs.iter().find(|(id, pc)| {
                        mark.cpu.as_str() == *id
                            && (mark.start & 0xFFFF..=mark.end & 0xFFFF).contains(pc)
                    });

                    if let Some((_, pc)) = hit {
                        hits.push(HookHit {
                            kind: mark.kind,
                            cpu: mark.cpu.clone(),
                            region: mark.region.clone(),
                            addr: *pc,
                            pc: *pc,
                        });
                    }
                }
                BreakpointKind::Read | BreakpointKind::Write => {
                    let set = match mark.kind {
                        BreakpointKind::Read => &access.reads,
                        _ => &access.writes,
                    };

                    let push = |addr: u32, hits: &mut Vec<HookHit>| {
                        hits.push(HookHit {
                            kind: mark.kind,
                            cpu: mark.cpu.clone(),
                            region: mark.region.clone(),
                            addr,
                            pc: main_pc,
                        });
                    };

                    match mark.region.as_ref() {
                        "bus" => {
                            for addr in mark.start..=mark.end {
                                if self
                                    .region_key("bus", addr)
                                    .is_some_and(|key| set.contains(&key))
                                {
                                    push(addr, &mut hits);
                                    if hits.len() >= MAX_HITS {
                                        break 'marks;
                                    }
                                }
                            }
                        }
                        region => {
                            for &key in set.iter() {
                                if (mark.start..=mark.end).contains(&key.1)
                                    && self.region_key(region, key.1) == Some(key)
                                {
                                    push(key.1, &mut hits);
                                    if hits.len() >= MAX_HITS {
                                        break 'marks;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        hits
    }

    pub(super) fn watch_hit(&self, breakpoints: &[BreakpointMark], access: &MemoryAccess) -> bool {
        breakpoints.iter().any(|m| {
            let set = match m.kind {
                BreakpointKind::Read => &access.reads,
                BreakpointKind::Write => &access.writes,
                _ => return false,
            };
            match m.region.as_ref() {
                "bus" => (m.start..=m.end).any(|addr| {
                    self.region_key("bus", addr)
                        .is_some_and(|key| set.contains(&key))
                }),
                _ => set.iter().any(|&key| {
                    (m.start..=m.end).contains(&key.1)
                        && self.region_key(&m.region, key.1) == Some(key)
                }),
            }
        })
    }

    pub(super) fn run_to(&mut self, target: &str) -> RunOutcome {
        const CAP: u64 = 2_000_000;

        let Some(target) = Target::from_id(target) else {
            return RunOutcome::Done;
        };

        enum Target {
            Scanline,
            Nmi,
            Irq,
            Spc,
            Dsp,
            Gsu,
        }

        impl Target {
            fn from_id(id: &str) -> Option<Self> {
                match id {
                    "scanline" => Some(Self::Scanline),
                    "nmi" => Some(Self::Nmi),
                    "irq" => Some(Self::Irq),
                    "spc" => Some(Self::Spc),
                    "dsp" => Some(Self::Dsp),
                    "gsu" => Some(Self::Gsu),
                    _ => None,
                }
            }
        }

        fn spc_state(spc: &spc700::Spc700) -> [u32; 2] {
            [
                (spc.pc as u32) << 16 | (spc.a as u32) << 8 | spc.x as u32,
                (spc.y as u32) << 16 | (spc.sp as u32) << 8 | spc.psw.raw() as u32,
            ]
        }

        enum Watch {
            Scanline(u64),
            Nmi(u64),
            Irq(u64),
            Spc([u32; 2]),
            Dsp(u64),
            Gsu,
        }

        let snapshot = |core: &Snes| match target {
            Target::Scanline => {
                Watch::Scanline(core.scheduler.scanline_at(core.scheduler.cycles()))
            }
            Target::Nmi => Watch::Nmi(core.cpu.interrupts.nmi),
            Target::Irq => Watch::Irq(core.cpu.interrupts.irq),
            Target::Spc => Watch::Spc(spc_state(&core.apu.spc700)),
            Target::Dsp => Watch::Dsp(core.apu.samples_generated()),
            Target::Gsu => Watch::Gsu,
        };

        let fired = |core: &Snes, watch: &Watch| match watch {
            Watch::Scanline(line) => core.scheduler.scanline_at(core.scheduler.cycles()) != *line,
            Watch::Nmi(count) => core.cpu.interrupts.nmi != *count,
            Watch::Irq(count) => core.cpu.interrupts.irq != *count,
            Watch::Spc(state) => spc_state(&core.apu.spc700) != *state,
            Watch::Dsp(count) => core.apu.samples_generated() != *count,
            Watch::Gsu => core.superfx.sfr.go(),
        };

        if matches!(target, Target::Gsu) && crate::superfx::debug_step(&mut self.core) {
            return RunOutcome::Breakpoint;
        }

        let start = self.core.scheduler.cycles();
        let mode = self.track_mode();
        let mut hit = false;

        self.last_access = self.captured(mode, |debug| {
            let watch = snapshot(&debug.core);
            while debug.core.scheduler.cycles() < start + CAP {
                debug.core.step();
                if fired(&debug.core, &watch) {
                    hit = true;
                    break;
                }
            }
        });

        if hit {
            RunOutcome::Breakpoint
        } else {
            RunOutcome::Done
        }
    }

    pub(super) fn run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        let watch = breakpoints
            .iter()
            .any(|m| matches!(m.kind, BreakpointKind::Read | BreakpointKind::Write));

        let events = !self.watch.is_empty();
        let frame_end = self.core.scheduler.frame_deadline();

        self.last_hits.clear();
        self.events.clear();
        self.event_baseline = self.event_snapshot();

        if watch {
            let mut access = MemoryAccess::default();
            while self.core.scheduler.cycles() < frame_end {
                let step_access = self.step_captured();
                access.merge(&step_access);

                let breakpoint =
                    self.watch_hit(breakpoints, &step_access) || self.execute_hit(breakpoints);
                let event = self.scan_events();

                if breakpoint {
                    self.last_hits = self.collect_hits(breakpoints, &step_access);
                    self.last_access = access;
                    return RunOutcome::Breakpoint;
                }

                if event {
                    self.last_access = access;
                    return self.event_outcome(frame_end);
                }
            }

            self.last_access = access;
            return RunOutcome::Done;
        }

        let mode = self.track_mode();
        let stepping = events || !breakpoints.is_empty();

        let mut outcome = RunOutcome::Done;
        let mut hits = Vec::new();
        self.last_access = self.captured(mode, |debug| {
            if !stepping {
                debug.core.run_frame();
                return;
            }

            while debug.core.scheduler.cycles() < frame_end {
                debug.core.step();

                let breakpoint = debug.execute_hit(breakpoints);
                let event = debug.scan_events();

                if breakpoint {
                    hits = debug.collect_hits(breakpoints, &MemoryAccess::default());
                    outcome = RunOutcome::Breakpoint;
                    break;
                }

                if event {
                    outcome = debug.event_outcome(frame_end);
                    break;
                }
            }
        });

        self.last_hits = hits;
        outcome
    }
}
