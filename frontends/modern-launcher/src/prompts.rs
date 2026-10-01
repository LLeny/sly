use std::sync::{LazyLock, OnceLock};

use crate::input::ActiveInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptIcon(u16);

struct PromptAsset {
    device: ActiveInput,
    controls: &'static [&'static str],
    png: &'static [u8],
}

pub struct PromptRaster {
    pub image: image::RgbaImage,
    /// Visible bounds within the padded source image, normalized.
    pub bounds: [f32; 4],
}

macro_rules! prompt {
    ($device:ident, $file:literal, $($control:literal),+) => {
        PromptAsset {
            device: ActiveInput::$device,
            controls: &[$($control),+],
            png: include_bytes!(concat!("../../../vendor/input-prompts/", $file)),
        }
    };
}

// Face buttons are named by position, so Xbox and Nintendo pads read the same.
const ASSETS: &[PromptAsset] = &[
    prompt!(Keyboard, "keyboard_arrow_left.png", "ArrowLeft"),
    prompt!(Keyboard, "keyboard_arrow_right.png", "ArrowRight"),
    prompt!(Keyboard, "keyboard_arrow_up.png", "ArrowUp"),
    prompt!(Keyboard, "keyboard_arrow_down.png", "ArrowDown"),
    prompt!(Keyboard, "keyboard_escape.png", "Escape"),
    prompt!(
        Keyboard,
        "keyboard_numpad_enter.png",
        "Enter",
        "NumpadEnter"
    ),
    prompt!(Keyboard, "keyboard_tab_icon.png", "Tab"),
    prompt!(Keyboard, "keyboard_f2.png", "F2"),
    prompt!(Keyboard, "keyboard_slash_forward.png", "Slash"),
    prompt!(Keyboard, "keyboard_backspace_icon.png", "Backspace"),
    prompt!(Keyboard, "keyboard_delete.png", "Delete"),
    prompt!(Keyboard, "keyboard_page_up.png", "PageUp"),
    prompt!(Keyboard, "keyboard_page_down.png", "PageDown"),
    prompt!(Keyboard, "keyboard_space.png", "Space"),
    prompt!(Keyboard, "keyboard_a.png", "A"),
    prompt!(Keyboard, "keyboard_b.png", "B"),
    prompt!(Keyboard, "keyboard_c.png", "C"),
    prompt!(Keyboard, "keyboard_d.png", "D"),
    prompt!(Keyboard, "keyboard_e.png", "E"),
    prompt!(Keyboard, "keyboard_f.png", "F"),
    prompt!(Keyboard, "keyboard_g.png", "G"),
    prompt!(Keyboard, "keyboard_h.png", "H"),
    prompt!(Keyboard, "keyboard_i.png", "I"),
    prompt!(Keyboard, "keyboard_j.png", "J"),
    prompt!(Keyboard, "keyboard_k.png", "K"),
    prompt!(Keyboard, "keyboard_l.png", "L"),
    prompt!(Keyboard, "keyboard_m.png", "M"),
    prompt!(Keyboard, "keyboard_n.png", "N"),
    prompt!(Keyboard, "keyboard_o.png", "O"),
    prompt!(Keyboard, "keyboard_p.png", "P"),
    prompt!(Keyboard, "keyboard_q.png", "Q"),
    prompt!(Keyboard, "keyboard_r.png", "R"),
    prompt!(Keyboard, "keyboard_s.png", "S"),
    prompt!(Keyboard, "keyboard_t.png", "T"),
    prompt!(Keyboard, "keyboard_u.png", "U"),
    prompt!(Keyboard, "keyboard_v.png", "V"),
    prompt!(Keyboard, "keyboard_w.png", "W"),
    prompt!(Keyboard, "keyboard_x.png", "X"),
    prompt!(Keyboard, "keyboard_y.png", "Y"),
    prompt!(Keyboard, "keyboard_z.png", "Z"),
    prompt!(Gamepad, "switch_buttons_down_outline.png", "South"),
    prompt!(Gamepad, "switch_buttons_right_outline.png", "East"),
    prompt!(Gamepad, "switch_buttons_left_outline.png", "West"),
    prompt!(Gamepad, "switch_buttons_up_outline.png", "North"),
    prompt!(Gamepad, "xbox_dpad.png", "D-pad"),
    prompt!(Gamepad, "xbox_dpad_up_outline.png", "DPadUp"),
    prompt!(Gamepad, "xbox_dpad_down_outline.png", "DPadDown"),
    prompt!(Gamepad, "xbox_dpad_left_outline.png", "DPadLeft"),
    prompt!(Gamepad, "xbox_dpad_right_outline.png", "DPadRight"),
    prompt!(Gamepad, "xbox_button_view.png", "Select"),
    prompt!(Gamepad, "xbox_button_menu.png", "Start"),
    prompt!(Gamepad, "xbox_stick_l_press.png", "LeftThumb"),
    prompt!(Gamepad, "xbox_stick_r_press.png", "RightThumb"),
    prompt!(Gamepad, "xbox_lb.png", "LeftTrigger"),
    prompt!(Gamepad, "xbox_rb.png", "RightTrigger"),
    prompt!(Gamepad, "xbox_lt.png", "LeftTrigger2"),
    prompt!(Gamepad, "xbox_rt.png", "RightTrigger2"),
    prompt!(Gamepad, "xbox_guide.png", "Mode"),
];

static RASTERS: LazyLock<Vec<OnceLock<PromptRaster>>> =
    LazyLock::new(|| ASSETS.iter().map(|_| OnceLock::new()).collect());

impl PromptIcon {
    pub fn for_control(control: &str, device: ActiveInput) -> Option<Self> {
        ASSETS
            .iter()
            .position(|asset| asset.device == device && asset.controls.contains(&control))
            .map(|index| Self(index as u16))
    }

    pub fn from_id(id: u16) -> Option<Self> {
        (usize::from(id) < ASSETS.len()).then_some(Self(id))
    }

    pub fn id(self) -> u16 {
        self.0
    }

    pub fn raster(self) -> &'static PromptRaster {
        let index = usize::from(self.0);

        RASTERS[index].get_or_init(|| {
            let mut image = image::load_from_memory(ASSETS[index].png)
                .expect("bundled Kenney prompt PNG is valid")
                .to_rgba8();

            let mut bounds = [image.width(), image.height(), 0, 0];

            for (x, y, pixel) in image.enumerate_pixels_mut() {
                if pixel.0[3] > 0 {
                    bounds = [
                        bounds[0].min(x),
                        bounds[1].min(y),
                        bounds[2].max(x + 1),
                        bounds[3].max(y + 1),
                    ];
                }

                pixel.0[..3].copy_from_slice(&[43, 39, 49]);
            }

            let [left, top, right, bottom] = bounds;

            PromptRaster {
                bounds: [
                    left as f32 / image.width() as f32,
                    top as f32 / image.height() as f32,
                    right as f32 / image.width() as f32,
                    bottom as f32 / image.height() as f32,
                ],
                image: image::imageops::crop_imm(&image, left, top, right - left, bottom - top)
                    .to_image(),
            }
        })
    }

    pub fn ink_height(self) -> f32 {
        let [_, top, _, bottom] = self.raster().bounds;
        (bottom - top) * 28.0
    }
}
