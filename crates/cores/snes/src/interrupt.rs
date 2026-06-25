#[derive(Default)]
pub struct Irq {
    pub nmitimen: NmiTimerEnable,
}

#[chapa::bitfield(u8, order = lsb0)]
#[derive(Default, Clone, Copy)]
pub struct NmiTimerEnable {
    #[bits(0)]
    pub joypad_autoread_enable: bool,

    #[bits(4..=5)]
    pub hv_timer_condition: u8,

    #[bits(7)]
    pub vblank_nmi_enable: bool,
}
