use core_model::AudioSpec;

pub(crate) struct AudioOut {
    spec: Option<AudioSpec>,
    player: Option<audio::AudioPlayer>,
    failed: bool,
}

impl AudioOut {
    pub(crate) fn new(spec: Option<AudioSpec>) -> Self {
        #[cfg_attr(target_family = "wasm", expect(unused_mut))]
        let mut out = Self {
            spec,
            player: None,
            failed: false,
        };

        #[cfg(not(target_family = "wasm"))]
        out.open();

        out
    }

    fn open(&mut self) {
        if self.player.is_some() || self.failed {
            return;
        }

        self.player = self.spec.and_then(|spec| {
            audio::AudioPlayer::new(spec)
                .inspect_err(|err| tracing::warn!("audio disabled: {err}"))
                .ok()
        });
        self.failed = self.player.is_none();
    }

    pub(crate) fn queue(&mut self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }

        self.open();
        if let Some(player) = self.player.as_mut() {
            player.queue(samples);
        }
    }
}
