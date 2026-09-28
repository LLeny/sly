use super::Width;

pub(super) struct Display;

impl Display {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn peek(&self, address: u32, width: Width) -> u32 {
        tracing::error!(
            address = format_args!("{:#010x}", address),
            width = format_args!("{:?}", width),
            "Peeking at PPU"
        );
        0xFF
    }

    pub(super) fn read(&mut self, address: u32, width: Width) -> u32 {
        tracing::error!(
            address = format_args!("{:#010x}", address),
            width = format_args!("{:?}", width),
            "Reading from PPU"
        );
        0xFF
    }

    pub(super) fn write(&mut self, _address: u32, _width: Width, _value: u32) {
        tracing::error!(
            address = format_args!("{:#010x}", _address),
            width = format_args!("{:?}", _width),
            value = format_args!("{:#010x}", _value),
            "Writing to PPU"
        );
    }
}
