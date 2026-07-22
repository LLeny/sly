#[derive(Debug, Clone, Copy)]
pub struct Spc700 {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub psw: Flags,
}

impl Spc700 {
    pub fn new(reset_vector: u16) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: reset_vector,
            psw: Flags::default(),
        }
    }

    pub fn ya(&self) -> u16 {
        ((self.y as u16) << 8) | self.a as u16
    }

    pub fn set_ya(&mut self, value: u16) {
        self.a = value as u8;
        self.y = (value >> 8) as u8;
    }
}

#[chapa::bitfield(u8, order = lsb0)]
#[derive(Default, Debug)]
pub struct Flags {
    #[bits(0, alias = "c")]
    pub carry: bool,

    #[bits(1, alias = "z")]
    pub zero: bool,

    #[bits(2, alias = "i")]
    pub irq_enable: bool,

    #[bits(3, alias = "h")]
    pub half_carry: bool,

    #[bits(4, alias = "b")]
    pub brk: bool,

    #[bits(5, alias = "p")]
    pub direct_page: bool,

    #[bits(6, alias = "v")]
    pub overflow: bool,

    #[bits(7, alias = "n")]
    pub negative: bool,
}
