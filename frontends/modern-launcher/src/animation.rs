use std::f32::consts::PI;

use crate::settings::ViewMode;

/// Timings in seconds.
#[derive(Clone, Copy, Debug)]
pub struct Presentation {
    pub navigation_rate: f32,
    pub flip_rate: f32,
    pub layout_rate: f32,
    pub closeup_duration: f32,
    pub iris_start: f32,
    pub iris_duration: f32,
    pub launch_duration: f32,
    pub gameplay_fade: f32,
    pub return_duration: f32,
    pub ambient_rate: f32,
    pub float_height: f32,
    pub float_rotation: f32,
}

pub const PRESENTATION: Presentation = Presentation {
    navigation_rate: 12.0,
    flip_rate: 16.0,
    layout_rate: 12.0,
    closeup_duration: 0.64,
    iris_start: 0.76,
    iris_duration: 0.68,
    launch_duration: 1.54,
    gameplay_fade: 0.26,
    return_duration: 0.58,
    ambient_rate: 0.55,
    float_height: 0.0025,
    float_rotation: 0.018,
};

const REDUCED_MOTION_DURATION: f32 = 0.15;

#[derive(Clone, Copy, Debug)]
struct Spring {
    value: f32,
    velocity: f32,
    target: f32,
}

impl Spring {
    fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            velocity: 0.0,
        }
    }

    fn snap(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
    }

    fn active(self) -> bool {
        (self.target - self.value).abs() > 0.0001 || self.velocity.abs() > 0.0001
    }

    fn advance(&mut self, dt: f32, rate: f32) {
        if !self.active() {
            self.snap();
            return;
        }

        let offset = self.value - self.target;
        let combined = self.velocity + rate * offset;
        let decay = (-rate * dt).exp();
        self.value = self.target + (offset + combined * dt) * decay;
        self.velocity = (self.velocity - rate * combined * dt) * decay;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Browsing,
    Launching,
    Playing,
    Returning,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TransitionFrame {
    pub cartridge_expansion: f32,
    /// 0 is fully open, 1 fully closed.
    pub iris_closed: f32,
    pub blackout_opacity: f32,
}

pub struct Animation {
    selection: Spring,
    flip: Spring,
    departing_flips: Vec<(usize, f32)>,
    layouts: [Spring; 3],
    pub phase: Phase,
    pub elapsed: f32,
    ambient_phase: f32,
    reduced_motion: bool,
    return_start: TransitionFrame,
}

impl Animation {
    pub fn new(selection: usize, layout: ViewMode, reduced_motion: bool) -> Self {
        Self {
            selection: Spring::new(selection as f32),
            flip: Spring::new(0.0),
            departing_flips: Vec::new(),
            layouts: std::array::from_fn(|index| Spring::new(f32::from(index == layout.index()))),
            phase: Phase::Browsing,
            elapsed: 0.0,
            ambient_phase: 0.0,
            reduced_motion,
            return_start: TransitionFrame::default(),
        }
    }

    pub fn selection(&self) -> f32 {
        self.selection.value
    }

    pub fn flip_angle_for(&self, index: usize) -> f32 {
        if index == self.selection.target as usize {
            self.flip.value * PI
        } else {
            self.departing_flips
                .iter()
                .find_map(|(entry, value)| (*entry == index).then_some(*value * PI))
                .unwrap_or(0.0)
        }
    }

    pub fn ambient_phase(&self) -> f32 {
        if self.reduced_motion {
            0.0
        } else {
            self.ambient_phase
        }
    }

    pub fn ambient_active(&self) -> bool {
        self.ambient_enabled() && self.phase == Phase::Browsing
    }

    pub fn ambient_enabled(&self) -> bool {
        !self.reduced_motion
    }

    pub fn set_ambient_phase(&mut self, phase: f32) {
        if phase.is_finite() {
            self.ambient_phase = phase.rem_euclid(2.0 * PI);
        }
    }

    pub fn view_weights(&self) -> [f32; 3] {
        self.layouts.map(|spring| spring.value)
    }

    pub fn select(&mut self, index: usize) {
        let previous = self.selection.target as usize;

        if previous != index {
            let incoming = self
                .departing_flips
                .iter()
                .find_map(|(entry, value)| (*entry == index).then_some(*value))
                .unwrap_or(0.0);

            self.departing_flips
                .retain(|(entry, _)| *entry != previous && *entry != index);

            if self.flip.value > 0.0001 && (previous as f32 - self.selection.value).abs() < 1.0 {
                self.departing_flips.push((previous, self.flip.value));
            }

            self.flip = Spring::new(incoming);
        }

        self.selection.target = index as f32;
        self.flip(false);

        if self.reduced_motion {
            self.selection.snap();
            self.departing_flips.clear();
        }
    }

    pub fn set_view(&mut self, layout: ViewMode) {
        for (index, spring) in self.layouts.iter_mut().enumerate() {
            spring.target = f32::from(index == layout.index());

            if self.reduced_motion {
                spring.snap();
            }
        }
    }

    pub fn flip(&mut self, flipped: bool) {
        self.flip.target = f32::from(flipped);

        if self.reduced_motion {
            self.flip.snap();
        }
    }

    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;

        if reduced {
            self.selection.snap();
            self.flip.snap();
            self.departing_flips.clear();

            for spring in &mut self.layouts {
                spring.snap();
            }
        }
    }

    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }

        self.selection.advance(dt, PRESENTATION.navigation_rate);
        self.departing_flips
            .retain(|(index, _)| (*index as f32 - self.selection.value).abs() < 1.0);
        self.flip.advance(dt, PRESENTATION.flip_rate);

        for spring in &mut self.layouts {
            spring.advance(dt, PRESENTATION.layout_rate);
        }

        self.elapsed += dt;

        if !self.reduced_motion && self.phase == Phase::Browsing {
            self.ambient_phase =
                (self.ambient_phase + dt * PRESENTATION.ambient_rate).rem_euclid(2.0 * PI);
        }

        if self.phase == Phase::Returning && self.elapsed >= self.return_duration() {
            self.phase = Phase::Browsing;
            self.elapsed = 0.0;
        }
    }

    pub fn begin_launch(&mut self) {
        self.phase = Phase::Launching;
        self.elapsed = 0.0;
    }

    pub fn launch_ready(&self) -> bool {
        self.phase == Phase::Launching
            && self.elapsed
                >= if self.reduced_motion {
                    REDUCED_MOTION_DURATION
                } else {
                    PRESENTATION.launch_duration
                }
    }

    pub fn enter_playing(&mut self) {
        self.phase = Phase::Playing;
        self.elapsed = 0.0;
    }

    pub fn return_to_library(&mut self) {
        self.return_start = TransitionFrame {
            iris_closed: 1.0,
            blackout_opacity: 1.0,
            ..Default::default()
        };

        self.phase = Phase::Returning;
        self.elapsed = 0.0;
    }

    pub fn cancel_launch(&mut self) {
        self.return_start = self.transition();
        self.phase = Phase::Returning;
        self.elapsed = 0.0;
    }

    fn return_duration(&self) -> f32 {
        if self.reduced_motion {
            REDUCED_MOTION_DURATION
        } else {
            PRESENTATION.return_duration
        }
    }

    pub fn transition(&self) -> TransitionFrame {
        let timings = PRESENTATION;

        match self.phase {
            Phase::Browsing => TransitionFrame::default(),
            Phase::Launching if self.reduced_motion => TransitionFrame {
                iris_closed: 1.0,
                blackout_opacity: smoothstep(self.elapsed / REDUCED_MOTION_DURATION),
                ..Default::default()
            },
            Phase::Launching => TransitionFrame {
                cartridge_expansion: smoothstep(self.elapsed / timings.closeup_duration),
                iris_closed: smoothstep(
                    (self.elapsed - timings.iris_start) / timings.iris_duration,
                ),
                blackout_opacity: 1.0,
            },
            Phase::Playing => {
                let progress = smoothstep(self.elapsed / timings.gameplay_fade);

                TransitionFrame {
                    iris_closed: 1.0,
                    blackout_opacity: 1.0 - progress,
                    ..Default::default()
                }
            }
            Phase::Returning => {
                let progress = smoothstep(self.elapsed / self.return_duration());

                TransitionFrame {
                    cartridge_expansion: self.return_start.cartridge_expansion * (1.0 - progress),
                    iris_closed: if self.reduced_motion {
                        1.0
                    } else {
                        self.return_start.iris_closed * (1.0 - progress)
                    },
                    blackout_opacity: self.return_start.blackout_opacity
                        * if self.reduced_motion {
                            1.0 - progress
                        } else {
                            1.0
                        },
                }
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.selection.active()
            || self.flip.active()
            || self.layouts.iter().any(|spring| spring.active())
            || match self.phase {
                Phase::Browsing => false,
                Phase::Launching => !self.launch_ready(),
                Phase::Playing => self.elapsed < PRESENTATION.gameplay_fade,
                Phase::Returning => true,
            }
    }
}

pub fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}
