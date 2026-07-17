use std::time::Duration;

use web_time::Instant;

const WINDOW: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct RunStats {
    window_start: Option<Instant>,
    last_tick: Option<Instant>,
    emu_total: Duration,
    emu_max: Duration,
    tick_total: Duration,
    wake_total: Duration,
    frames: u32,
    ticks: u32,
}

impl RunStats {
    pub fn record_frame(&mut self, emu: Duration) {
        self.emu_total += emu;
        self.emu_max = self.emu_max.max(emu);
        self.frames += 1;
    }

    pub fn record_tick(
        &mut self,
        wake_late: Duration,
        buffered: Option<Duration>,
    ) -> Option<String> {
        let now = Instant::now();

        if let Some(last) = self.last_tick {
            self.tick_total += now - last;
            self.wake_total += wake_late;
            self.ticks += 1;
        }

        self.last_tick = Some(now);

        let start = *self.window_start.get_or_insert(now);
        let elapsed = now - start;
        if elapsed < WINDOW || self.ticks == 0 || self.frames == 0 {
            return None;
        }

        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let fps = self.frames as f64 / elapsed.as_secs_f64();
        let audio = buffered.map_or(String::new(), |b| format!(" | buf {:.0}ms", ms(b)));
        let summary = format!(
            "core {:.1}ms (max {:.1}) | tick {:.1}ms | wake +{:.1}ms{audio} | {:.1} fps",
            ms(self.emu_total) / self.frames as f64,
            ms(self.emu_max),
            ms(self.tick_total) / self.ticks as f64,
            ms(self.wake_total) / self.ticks as f64,
            fps
        );

        *self = Self {
            last_tick: Some(now),
            window_start: Some(now),
            ..Self::default()
        };

        Some(summary)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
