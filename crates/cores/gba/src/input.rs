use core_model::{Button, ControllerDescriptor};

use crate::mmio::{MmioMap, Register};

const BUTTONS: [Button; 10] = [
    Button::A,
    Button::B,
    Button::Select,
    Button::Start,
    Button::Right,
    Button::Left,
    Button::Up,
    Button::Down,
    Button::R,
    Button::L,
];

// Active low!
#[chapa::bitfield(u16, order = lsb0)]
pub(crate) struct KeyInput {
    #[bits(0..=9)]
    released: u16,
}

const KEYINPUT: Register<KeyInput> = Register::new(
    0x0400_0130,
    "KEYINPUT",
    "Keypad",
    |bus| bus.input.keyinput,
    |bus| &mut bus.input.keyinput,
)
.read_mask(KeyInput::RELEASED_MASK)
.read_only();

pub(crate) struct Input {
    pub(crate) keyinput: KeyInput,
}

impl Input {
    pub(crate) fn new() -> Self {
        Self {
            keyinput: KeyInput::zeroed().with_released(KeyInput::RELEASED_MASK),
        }
    }

    pub(crate) fn set_button(&mut self, port: usize, button: Button, pressed: bool) {
        if port != 0 {
            return;
        }
        let Some(bit) = BUTTONS.iter().position(|&candidate| candidate == button) else {
            return;
        };

        let mask = 1u16 << bit;
        let released = self.keyinput.released();
        self.keyinput.set_released(if pressed {
            released & !mask
        } else {
            released | mask
        });
    }

    pub(crate) fn controllers(&self) -> Vec<ControllerDescriptor> {
        vec![ControllerDescriptor {
            port: 0,
            name: "GBA Keypad".into(),
            buttons: BUTTONS.to_vec(),
        }]
    }
}

pub(crate) fn register(map: &mut MmioMap) {
    map.register(&KEYINPUT);
}
