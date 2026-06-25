macro_rules! register_mmio {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident @ $addr:literal { $($body:tt)* }
    ) => {
        register_mmio!(@emit u8, $(#[$meta])* $vis struct $name @ $addr { $($body)* });
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident: $repr:ident @ $addr:literal { $($body:tt)* }
    ) => {
        register_mmio!(@emit $repr, $(#[$meta])* $vis struct $name @ $addr { $($body)* });
    };
    (@emit $repr:ident,
        $(#[$meta:meta])*
        $vis:vis struct $name:ident @ $addr:literal { $($body:tt)* }
    ) => {
        #[chapa::bitfield($repr, order = lsb0)]
        #[derive(Default, Clone, Copy)]
        $(#[$meta])*
        $vis struct $name { $($body)* }

        impl $name {
            pub const ADDRESS: u16 = $addr;
        }
    };
}

macro_rules! mmio_passthrough {
    ($name:ident => $($field:ident).+) => {
        impl crate::mmio::MmioReg for $name {
            fn read(snes: &crate::Snes) -> u8 {
                snes.$($field).+.raw()
            }

            fn write(snes: &mut crate::Snes, value: u8) {
                snes.$($field).+ = Self::from_raw(value);
            }
        }
    };
}

pub(crate) use mmio_passthrough;
pub(crate) use register_mmio;
