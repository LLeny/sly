macro_rules! register_mmio {
    (
        #[doc = $doc:literal]
        $(#[$meta:meta])*
        $vis:vis struct $name:ident: $repr:ident @ $addr:literal { $($body:tt)* }
    ) => {
        register_mmio!(@emit $repr, $doc, #[doc = $doc] $(#[$meta])* $vis struct $name @ $addr { $($body)* });
    };
    (
        #[doc = $doc:literal]
        $(#[$meta:meta])*
        $vis:vis struct $name:ident @ $addr:literal { $($body:tt)* }
    ) => {
        register_mmio!(@emit u8, $doc, #[doc = $doc] $(#[$meta])* $vis struct $name @ $addr { $($body)* });
    };
    (@emit $repr:ident, $doc:literal,
        $(#[$meta:meta])*
        $vis:vis struct $name:ident @ $addr:literal { $($body:tt)* }
    ) => {
        #[chapa::bitfield($repr, order = lsb0)]
        #[derive(Default, Clone, Copy)]
        $(#[$meta])*
        $vis struct $name { $($body)* }

        impl $name {
            pub const ADDRESS: u16 = $addr;
            pub const NAME: &'static str = $doc.trim_ascii();
        }

        inventory::submit! {
            $crate::mmio::MmioSymbol {
                addr: $name::ADDRESS,
                name: $name::NAME,
                bits: (::core::mem::size_of::<$repr>() * 8) as u8,
                fields: $name::FIELDS,
            }
        }
    };
}

macro_rules! register_mmio_raw {
    ($($addr:literal => $name:literal),+ $(,)?) => {
        $(
            inventory::submit! {
                $crate::mmio::MmioSymbol { addr: $addr, name: $name, bits: 8, fields: &[] }
            }
        )+
    };
}

macro_rules! mmio_passthrough {
    ($name:ident => $($field:ident).+) => {
        impl crate::mmio::MmioReg for $name {
            fn read<M: crate::mmio::MemoryMap>(snes: &crate::Snes<M>) -> u8 {
                snes.$($field).+.raw()
            }

            fn write<M: crate::mmio::MemoryMap>(snes: &mut crate::Snes<M>, value: u8) {
                snes.$($field).+ = Self::from_raw(value);
            }

            fn peek<M: crate::mmio::MemoryMap>(snes: &crate::Snes<M>) -> u8 {
                snes.$($field).+.raw()
            }
        }
    };
}

pub(crate) use mmio_passthrough;
pub(crate) use register_mmio;
pub(crate) use register_mmio_raw;
