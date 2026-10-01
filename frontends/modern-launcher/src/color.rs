use std::sync::LazyLock;

static TO_LINEAR: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|value| srgb_to_linear(value as f32 / 255.0)));

pub fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub fn srgb8_to_linear(value: u8) -> f32 {
    TO_LINEAR[usize::from(value)]
}

/// Rec. 709 relative luminance of linear RGB.
pub fn luminance([red, green, blue]: [f32; 3]) -> f32 {
    red * 0.2126 + green * 0.7152 + blue * 0.0722
}
