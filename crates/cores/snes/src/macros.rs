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
        #[derive(Default)]
        $(#[$meta])*
        $vis struct $name { $($body)* }

        $crate::macros::impl_bitfield_serde!($name);

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

macro_rules! register_mmio_family {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
        symbols { $($addr:literal => $sym:literal),+ $(,)? }
    ) => {
        #[chapa::bitfield(u8, order = lsb0)]
        $(#[$meta])*
        $vis struct $name { $($body)* }

        $crate::macros::impl_bitfield_serde!($name);

        $(
            inventory::submit! {
                $crate::mmio::MmioSymbol {
                    addr: $addr,
                    name: $sym,
                    bits: (::core::mem::size_of::<$name>() * 8) as u8,
                    fields: $name::FIELDS,
                }
            }
        )+
    };
}

macro_rules! register_mmio_raw {
    ($($addr:expr => $name:expr),+ $(,)?) => {
        $(
            inventory::submit! {
                $crate::mmio::MmioSymbol { addr: $addr, name: $name, bits: 8, fields: &[] }
            }
        )+
    };
}

macro_rules! impl_bitfield_serde {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl serde::Serialize for $ty {
                fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                    serde::Serialize::serialize(&<$ty as chapa::BitField>::raw(self), s)
                }
            }

            impl<'de> serde::Deserialize<'de> for $ty {
                fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    Ok(<$ty as chapa::BitField>::from_raw(
                        serde::Deserialize::deserialize(d)?,
                    ))
                }
            }
        )+
    };
}

pub(crate) use impl_bitfield_serde;
pub(crate) use register_mmio;
pub(crate) use register_mmio_family;
pub(crate) use register_mmio_raw;
