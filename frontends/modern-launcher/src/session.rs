use std::io;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use core_model::{Button, ControllerDescriptor, DisplayRotation};

use crate::library::{LibraryEntry, read_rom};
use crate::worker::LatestJob;

const MAX_CATCH_UP: u32 = 3;
const BACKUP_INTERVAL: Duration = Duration::from_secs(60);

pub type SessionLoader = LatestJob<(LibraryEntry, Option<PathBuf>), Result<LoadedGame, String>>;

impl SessionLoader {
    pub fn new() -> io::Result<Self> {
        Self::spawn("launcher-core-load", |(entry, save_root), _| {
            std::panic::catch_unwind(AssertUnwindSafe(|| LoadedGame::load(&entry, save_root)))
                .unwrap_or_else(|_| {
                    Err("Core loader failed unexpectedly while opening this ROM".into())
                })
        })
    }
}

pub struct LoadedGame {
    core: core_loader::PlayerCore,
    storage: storage::Session<storage::FsStore>,
    interval: Duration,
    rotation: DisplayRotation,
    pub warnings: Vec<String>,
}

impl LoadedGame {
    fn load(entry: &LibraryEntry, save_root: Option<PathBuf>) -> Result<Self, String> {
        let bytes = read_rom(&entry.path)?;

        // The file may have changed since the scan; don't load it under the old identity.
        if core_loader::identity(entry.system, &bytes).sha1 != entry.sha1 {
            return Err(
                "ROM changed since the library was scanned; rescan before launching".into(),
            );
        }

        let cartridge = core_model::RomFile::new()
            .data(bytes)
            .filename(entry.path.display().to_string());
        let mut core =
            core_loader::load(entry.system, cartridge).map_err(|error| error.to_string())?;

        let interval = core
            .frame_interval()
            .filter(|interval| !interval.is_zero())
            .ok_or_else(|| "Core does not report a valid frame interval".to_string())?;

        let store = save_root
            .map(storage::FsStore::with_root)
            .unwrap_or_default();

        let storage = storage::Session::new(store, entry.game_id());
        let mut warnings = Vec::new();

        if let Err(error) = storage.restore_save_ram(&mut *core) {
            // Further save RAM writes are disabled for this session.
            warnings.push(format!(
                "Backup restore failed: {error}. Existing backup is protected."
            ));
        }

        let rotation = core
            .cartridge_info()
            .map(|info| info.display_rotation)
            .unwrap_or_default();

        Ok(Self {
            core,
            storage,
            interval,
            rotation,
            warnings,
        })
    }

    /// Call on the window thread, which must own audio on some platforms.
    pub fn activate(self, volume: f32) -> GameSession {
        let controllers = self.core.controllers();

        let mut session = GameSession {
            core: self.core,
            storage: self.storage,
            controllers,
            audio: None,
            audio_samples: Vec::new(),
            clock: FrameClock::new(),
            interval: self.interval,
            next_backup: Instant::now() + BACKUP_INTERVAL,
            sequence: 1,
            rotation: self.rotation,
            paused: false,
            volume: sanitize_volume(volume),
            warnings: self.warnings,
        };

        session.open_audio();
        session
    }
}

/// Straight sRGB RGBA8 pixels.
pub struct VideoFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub sequence: u64,
    pub display_aspect: f32,
    pub rotation: DisplayRotation,
    pub bytes: &'a [u8],
}

pub struct GameSession {
    core: core_loader::PlayerCore,
    storage: storage::Session<storage::FsStore>,
    controllers: Vec<ControllerDescriptor>,
    audio: Option<audio::AudioPlayer>,
    audio_samples: Vec<i16>,
    clock: FrameClock,
    interval: Duration,
    next_backup: Instant,
    sequence: u64,
    rotation: DisplayRotation,
    paused: bool,
    volume: f32,
    warnings: Vec<String>,
}

impl GameSession {
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.paused {
            self.clock.reset();
            return false;
        }

        let interval = self.core.frame_interval().unwrap_or(self.interval);
        let due = self.clock.frames_due(now, interval);

        for _ in 0..due {
            self.core.run_frame();
            self.sequence = self.sequence.wrapping_add(1);
            self.audio_samples.clear();
            self.core.drain_audio(&mut self.audio_samples);

            if let Some(audio) = &mut self.audio {
                if self.volume < 1.0 {
                    for sample in &mut self.audio_samples {
                        *sample = (*sample as f32 * self.volume).round() as i16;
                    }
                }

                audio.queue(&self.audio_samples);

                if self.sequence == 2 {
                    tracing::debug!(
                        samples = self.audio_samples.len(),
                        volume = self.volume,
                        "Initial core audio queued"
                    );
                }
            }
        }

        if now >= self.next_backup {
            self.next_backup = now + BACKUP_INTERVAL;

            if let Err(error) = self.persist() {
                self.warnings
                    .push(format!("Periodic backup failed: {error}"));
            }
        }

        due > 0
    }

    pub fn next_frame_due(&self) -> Option<Instant> {
        if self.paused { None } else { self.clock.next }
    }

    pub fn frame(&self) -> Option<VideoFrame<'_>> {
        let frame = self
            .core
            .software_frame()
            .filter(|frame| frame.is_valid())?;

        let display_aspect = self
            .core
            .display_aspect()
            .unwrap_or_else(|| frame.width as f32 / frame.height.max(1) as f32);

        Some(VideoFrame {
            width: frame.width,
            height: frame.height,
            stride: frame.width.checked_mul(4)?,
            sequence: self.sequence,
            display_aspect,
            rotation: self.rotation,
            bytes: frame.bytes(),
        })
    }

    pub fn set_button(&mut self, port: usize, button: Button, pressed: bool) {
        if (self.paused && pressed)
            || !self
                .controllers
                .iter()
                .any(|controller| controller.port == port && controller.supports(button))
        {
            return;
        }

        self.core.set_button(port, button, pressed);
    }

    pub fn clear_inputs(&mut self) {
        for controller in &self.controllers {
            for &button in &controller.buttons {
                self.core.set_button(controller.port, button, false);
            }
        }
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        if self.paused == paused {
            return;
        }

        self.paused = paused;
        self.clock.reset();
        self.clear_inputs();
        // Drop queued samples so resuming doesn't replay stale audio.
        self.audio = None;
        self.audio_samples.clear();

        if paused {
            if let Err(error) = self.persist() {
                self.warnings
                    .push(format!("Backup on pause failed: {error}"));
            }
        } else {
            self.open_audio();
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        let was_muted = self.muted();
        self.volume = sanitize_volume(volume);

        if self.muted() != was_muted {
            self.audio = None;
            self.open_audio();
        }
    }

    fn muted(&self) -> bool {
        self.volume == 0.0
    }

    fn open_audio(&mut self) {
        if self.muted() || self.paused {
            return;
        }

        if let Some(spec) = self.core.audio_spec() {
            match audio::AudioPlayer::new(spec) {
                Ok(audio) => {
                    tracing::debug!(?spec, "Audio stream opened");
                    self.audio = Some(audio);
                }
                Err(error) => self.warnings.push(format!("Audio unavailable: {error}")),
            }
        }
    }

    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    pub fn quick_save(&self) -> Result<(), String> {
        self.storage
            .save_state(&*self.core, storage::Slot::Quick)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn quick_load(&mut self) -> Result<bool, String> {
        let loaded = self
            .storage
            .load_state(&mut *self.core, storage::Slot::Quick)
            .map_err(|error| error.to_string())?;

        if loaded {
            self.clear_inputs();
            self.clock.reset();
            self.sequence = self.sequence.wrapping_add(1);
            self.audio = None;
            self.audio_samples.clear();
            self.open_audio();
        }

        Ok(loaded)
    }

    pub fn persist(&self) -> Result<bool, String> {
        self.storage
            .persist_save_ram(&*self.core)
            .map_err(|error| error.to_string())
    }
}

impl Drop for GameSession {
    fn drop(&mut self) {
        self.clear_inputs();
        self.audio = None;

        if let Err(error) = self.persist() {
            tracing::warn!(%error, "Final save RAM persistence failed");
        }
    }
}

fn sanitize_volume(volume: f32) -> f32 {
    if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

struct FrameClock {
    next: Option<Instant>,
}

impl FrameClock {
    fn new() -> Self {
        Self { next: None }
    }

    fn reset(&mut self) {
        self.next = None;
    }

    fn frames_due(&mut self, now: Instant, interval: Duration) -> u32 {
        let mut next = self.next.unwrap_or(now);
        let mut due = 0;

        while now >= next && due < MAX_CATCH_UP {
            due += 1;
            next += interval;
        }

        if now >= next {
            next = now + interval;
        }

        self.next = Some(next);
        due
    }
}
