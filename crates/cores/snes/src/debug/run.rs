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

pub(super) struct Sa1Stops<'a> {
    cpu: CpuId,
    marks: Vec<&'a BreakpointMark>,
}

impl<'a> Sa1Stops<'a> {
    fn new(breakpoints: &'a [BreakpointMark]) -> Self {
        let cpu = CpuId::from(sa1::CPU);
        let marks = breakpoints
            .iter()
            .filter(|mark| mark.kind == BreakpointKind::Execute && mark.cpu == cpu)
            .collect();

        Self { cpu, marks }
    }

    fn covers(&self, pc: u32) -> bool {
        self.marks
            .iter()
            .any(|mark| mark.covers(BreakpointKind::Execute, &self.cpu, "sa1", pc, 1))
    }
}

impl SnesDebug {
    fn cpu_pcs(&self) -> [(&'static str, u32); 4] {
        [
            (CPU_MAIN, self.pc_of(CPU_MAIN)),
            (CPU_SPC, self.pc_of(CPU_SPC)),
            (sa1::CPU, self.pc_of(sa1::CPU)),
            (superfx::CPU_GSU, self.pc_of(superfx::CPU_GSU)),
        ]
    }

    pub(super) fn execute_hit(&self, breakpoints: &[BreakpointMark]) -> bool {
        let pcs = self.cpu_pcs();

        breakpoints
            .iter()
            .any(|m| pcs.iter().any(|&(id, pc)| executes_at(m, id, pc)))
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
        self.captured(self.track_mode(), |debug| debug.core.step())
    }

    fn step_stopping_at(&mut self, sa1: &Sa1Stops<'_>) {
        self.core
            .step_until_sa1(|cpu| sa1.covers(Address::new(cpu.program_bank, cpu.pc).as_24bit()));
    }

    pub(super) fn track_mode(&self) -> TrackMode {
        match self.tracking {
            AccessTracking::Off => TrackMode::Off,
            AccessTracking::Latest => TrackMode::LastOnly,
            AccessTracking::Highlights => TrackMode::Full,
        }
    }

    pub(super) fn event_outcome(&self, frame_end: u64) -> RunOutcome {
        if self.core.scheduler.cycles() >= frame_end {
            RunOutcome::FrameComplete
        } else {
            RunOutcome::Event
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
        let pc = self.pc(&CPU_MAIN.into());
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
        let mut hits = Vec::new();
        for (id, pc) in self.cpu_pcs() {
            if breakpoints.iter().any(|mark| executes_at(mark, id, pc)) {
                let cpu = CpuId::from(id);
                hits.push(HookHit {
                    kind: BreakpointKind::Execute,
                    cpu: cpu.clone(),
                    region: cpu_space(id).into(),
                    addr: pc,
                    pc,
                    value: 0,
                    width: AccessWidth::Byte,
                    source: AccessSource::Cpu(cpu),
                });
            }
        }

        if access.events.is_empty() {
            return hits;
        }

        let (main, spc) = (CpuId::from(CPU_MAIN), CpuId::from(CPU_SPC));
        for event in &access.events {
            let cpu = if event.key.0 == MemKind::Spc {
                &spc
            } else {
                &main
            };

            let kind = if event.write {
                BreakpointKind::Write
            } else {
                BreakpointKind::Read
            };

            let mut emitted_regions: Vec<&str> = Vec::new();
            for mark in breakpoints {
                if mark.kind != kind || emitted_regions.contains(&mark.region.as_ref()) {
                    continue;
                }

                let addr = if mark.region.as_ref() == "bus" {
                    if event.key.0 == MemKind::Spc {
                        continue;
                    }

                    event.address
                } else {
                    if self.region_key(&mark.region, event.key.1) != Some(event.key) {
                        continue;
                    }

                    event.key.1
                };

                if !mark.covers(kind, cpu, &mark.region, addr, 1) {
                    continue;
                }

                emitted_regions.push(&mark.region);
                hits.push(HookHit {
                    kind,
                    cpu: cpu.clone(),
                    region: mark.region.clone(),
                    addr,
                    pc: event.pc,
                    value: u32::from(event.value),
                    width: AccessWidth::Byte,
                    source: if event.dma {
                        AccessSource::Dma
                    } else {
                        AccessSource::Cpu(cpu.clone())
                    },
                });
            }
        }

        hits
    }

    pub(super) fn run_to(&mut self, target: &str) -> RunOutcome {
        const CAP: u64 = 2_000_000;
        if !self
            .step_targets()
            .iter()
            .any(|descriptor| descriptor.id.as_ref() == target)
        {
            return RunOutcome::Unsupported;
        }

        let Some(target) = Target::from_id(target) else {
            return RunOutcome::Unsupported;
        };

        enum Target {
            Scanline,
            Nmi,
            Irq,
            Spc,
            Dsp,
            Sa1,
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
                    "sa1" => Some(Self::Sa1),
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

        fn sa1_state(sa1: &crate::sa1::Sa1) -> [u32; 3] {
            [
                (u32::from(sa1.cpu.program_bank) << 16) | u32::from(sa1.cpu.pc),
                (u32::from(sa1.cpu.acc) << 16) | u32::from(sa1.cpu.index_x),
                (u32::from(sa1.cpu.index_y) << 16) | u32::from(sa1.cpu.stack_pointer),
            ]
        }

        enum Watch {
            Scanline(u64),
            Nmi(u64),
            Irq(u64),
            Spc([u32; 2]),
            Dsp(u64),
            Sa1([u32; 3]),
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
            Target::Sa1 => Watch::Sa1(sa1_state(&core.sa1)),
            Target::Gsu => Watch::Gsu,
        };

        let fired = |core: &Snes, watch: &Watch| match watch {
            Watch::Scanline(line) => core.scheduler.scanline_at(core.scheduler.cycles()) != *line,
            Watch::Nmi(count) => core.cpu.interrupts.nmi != *count,
            Watch::Irq(count) => core.cpu.interrupts.irq != *count,
            Watch::Spc(state) => spc_state(&core.apu.spc700) != *state,
            Watch::Dsp(count) => core.apu.samples_generated() != *count,
            Watch::Sa1(state) => sa1_state(&core.sa1) != *state,
            Watch::Gsu => core.superfx.sfr.go(),
        };

        if matches!(target, Target::Sa1) && crate::sa1::debug_step(&mut self.core) {
            return RunOutcome::TargetReached;
        }

        if matches!(target, Target::Gsu) && crate::superfx::debug_step(&mut self.core) {
            return RunOutcome::TargetReached;
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
            RunOutcome::TargetReached
        } else {
            RunOutcome::BudgetExhausted
        }
    }

    pub(super) fn run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        let sa1_stops = Sa1Stops::new(breakpoints);
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
                let step_access = self.captured(TrackMode::Transactions, |debug| {
                    debug.step_stopping_at(&sa1_stops)
                });

                access.merge_highlights(&step_access);

                let hits = self.collect_hits(breakpoints, &step_access);
                let event = self.scan_events();

                if !hits.is_empty() {
                    self.last_hits = hits;
                    self.last_access = access;
                    return RunOutcome::Breakpoint;
                }

                if event {
                    self.last_access = access;
                    return self.event_outcome(frame_end);
                }
            }

            self.last_access = access;
            return RunOutcome::FrameComplete;
        }

        let mode = self.track_mode();
        let stepping = events || !breakpoints.is_empty();

        let mut outcome = RunOutcome::FrameComplete;
        let mut hits = Vec::new();
        self.last_access = self.captured(mode, |debug| {
            if !stepping {
                debug.core.run_frame();
                return;
            }

            while debug.core.scheduler.cycles() < frame_end {
                debug.step_stopping_at(&sa1_stops);

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

fn executes_at(mark: &BreakpointMark, cpu: &str, pc: u32) -> bool {
    mark.kind == BreakpointKind::Execute
        && mark.cpu.as_str() == cpu
        && mark.region.as_ref() == cpu_space(cpu)
        && (mark.start..=mark.end).contains(&pc)
}

fn cpu_space(cpu: &str) -> &'static str {
    match cpu {
        CPU_SPC => "spc",
        sa1::CPU => "sa1",
        superfx::CPU_GSU => "gsu",
        _ => "bus",
    }
}
