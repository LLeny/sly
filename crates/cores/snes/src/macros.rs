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

pub(crate) use register_mmio;
