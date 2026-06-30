#[chapa::bitfield(u8, order = lsb0)]
#[derive(Default, Clone, Copy)]
pub struct Control {
    #[bits(0)]
    pub enable_timer0: bool,

    #[bits(1)]
    pub enable_timer1: bool,

    #[bits(2)]
    pub enable_timer2: bool,

    #[bits(4)]
    pub clear_cpuio0: bool,

    #[bits(5)]
    pub clear_cpuio1: bool,

    #[bits(7, default = true)]
    pub ipl_rom_enable: bool,
}
