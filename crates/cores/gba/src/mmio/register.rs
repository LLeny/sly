use chapa::{BitField, BitStorage, FieldInfo, FieldKind, Reflect};

use crate::bus::{BusView, CpuBus};

#[derive(Clone, Copy, Debug)]
pub struct RegisterInfo {
    pub address: u32,
    pub name: &'static str,
    pub group: &'static str,
    pub bits: u8,
    pub read_mask: u32,
    pub write_mask: u32,
    pub fields: &'static [FieldInfo],
}

type Backing<T> = for<'a, 'bus> fn(&'a mut CpuBus<'bus>) -> &'a mut T;

pub struct Register<T: BitField> {
    info: RegisterInfo,
    read_mask: Option<T::Storage>,
    write_mask: Option<T::Storage>,
    get: fn(&BusView<'_>) -> T,
    backing: Backing<T>,
    read: Option<fn(&mut CpuBus<'_>, T::Storage) -> T>,
    write: Option<fn(&mut CpuBus<'_>, T, T::Storage)>,
}

impl<T: BitField + Reflect> Register<T>
where
    T::Storage: Into<u32>,
{
    pub const fn new(
        address: u32,
        name: &'static str,
        group: &'static str,
        get: fn(&BusView<'_>) -> T,
        backing: Backing<T>,
    ) -> Self {
        let bits = T::Storage::BITS as u8;
        let fields = match T::REFLECT {
            FieldKind::Struct(fields) => fields,
            _ => panic!("a register must be backed by a chapa bitfield struct"),
        };

        Self {
            info: RegisterInfo {
                address,
                name,
                group,
                bits,
                read_mask: mask(bits),
                write_mask: mask(bits),
                fields,
            },
            read_mask: None,
            write_mask: None,
            get,
            backing,
            read: None,
            write: None,
        }
    }

    pub const fn read_mask(mut self, value: T::Storage) -> Self {
        self.read_mask = Some(value);
        self
    }

    pub const fn write_mask(mut self, value: T::Storage) -> Self {
        self.write_mask = Some(value);
        self
    }

    pub const fn read_only(self) -> Self {
        self.write_mask(T::Storage::ZERO)
    }

    pub const fn read_with(mut self, read: fn(&mut CpuBus<'_>, T::Storage) -> T) -> Self {
        self.read = Some(read);
        self
    }

    pub const fn write_with(mut self, write: fn(&mut CpuBus<'_>, T, T::Storage)) -> Self {
        self.write = Some(write);
        self
    }
}

impl<T: BitField> Register<T>
where
    T::Storage: Into<u32>,
{
    fn resolved_read_mask(&self) -> u32 {
        self.read_mask.map_or(self.info.read_mask, Into::into)
    }

    fn resolved_write_mask(&self) -> u32 {
        self.write_mask.map_or(self.info.write_mask, Into::into)
    }
}

pub(crate) trait RegisterAccess: Sync {
    fn info(&self) -> RegisterInfo;
    fn inspect(&self, bus: &BusView<'_>) -> u32;
    fn peek(&self, bus: &BusView<'_>) -> u32;
    fn read(&self, bus: &mut CpuBus<'_>, lanes: u32) -> u32;
    fn write(&self, bus: &mut CpuBus<'_>, value: u32, lanes: u32);
}

impl<T: BitField + Reflect> RegisterAccess for Register<T>
where
    T::Storage: Into<u32> + Sync,
{
    fn info(&self) -> RegisterInfo {
        RegisterInfo {
            read_mask: self.resolved_read_mask(),
            write_mask: self.resolved_write_mask(),
            ..self.info
        }
    }

    fn inspect(&self, bus: &BusView<'_>) -> u32 {
        (self.get)(bus).raw().into()
    }

    fn peek(&self, bus: &BusView<'_>) -> u32 {
        self.inspect(bus) & self.resolved_read_mask()
    }

    fn read(&self, bus: &mut CpuBus<'_>, lanes: u32) -> u32 {
        let read_mask = self.resolved_read_mask();
        if read_mask & lanes == 0 {
            return 0;
        }

        let value = match self.read {
            Some(read) => read(bus, T::Storage::from_u128(lanes.into())).raw().into(),
            None => self.inspect(&bus.view()),
        };
        value & read_mask
    }

    fn write(&self, bus: &mut CpuBus<'_>, value: u32, lanes: u32) {
        let mask = lanes & self.resolved_write_mask();
        if mask == 0 {
            return;
        }

        if let Some(write) = self.write {
            write(
                bus,
                T::from_raw(T::Storage::from_u128(value.into())),
                T::Storage::from_u128(mask.into()),
            );
        } else {
            let backing = (self.backing)(bus);
            let old = backing.raw().into();
            *backing = T::from_raw(T::Storage::from_u128(
                ((old & !mask) | (value & mask)).into(),
            ));
        }
    }
}

pub(crate) const fn mask(bits: u8) -> u32 {
    u32::MAX >> (32 - bits)
}
