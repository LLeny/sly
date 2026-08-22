use crate::SnesBus;
use crate::mmio::{Address, MemoryMap};
use crate::superfx::bitmap::{self, PixelCache, Screen};
use crate::superfx::decoder::{
    self as decoder, AluKind, BankKind, BitmapKind, BranchesKind, ByteOpsKind, CallKind, HaltKind,
    Instruction, JumpKind, LoadImmKind, LoadRamKind, LoopCtlKind, MoveRegKind, MuldivKind,
    PrefixAltKind, PrefixRegKind, RegStepKind, RomReadKind, ShiftKind, StoreRamKind,
};

struct Interpreter<'a, M: MemoryMap> {
    snes: &'a mut SnesBus<M>,
    r15_changed: bool,
    cycles: u64,
}

pub(super) fn step<M: MemoryMap>(snes: &mut SnesBus<M>) -> u64 {
    Interpreter::new(snes).step()
}

pub(super) fn drain_buffers<M: MemoryMap>(snes: &mut SnesBus<M>) {
    let pending = snes
        .superfx
        .rom_buffer
        .delay
        .max(snes.superfx.ram_buffer.delay);

    if pending > 0 {
        Interpreter::new(snes).tick(pending as u64);
    }
}

impl<'a, M: MemoryMap> Interpreter<'a, M> {
    fn new(snes: &'a mut SnesBus<M>) -> Self {
        Self {
            snes,
            r15_changed: false,
            cycles: 0,
        }
    }

    fn step(&mut self) -> u64 {
        let combo = decoder::mode_combo(&self.snes.superfx.sfr);

        let op = self.read_opcode();
        decoder::run_ops(combo, self, op);

        if !self.r15_changed {
            let fx = &mut self.snes.superfx;
            fx.r[15] = fx.r[15].wrapping_add(1);
        }

        self.cycles
    }

    fn bus_cost(&self) -> u64 {
        self.snes.superfx.bus_cost()
    }

    fn cache_cost(&self) -> u64 {
        self.snes.superfx.cache_cost()
    }

    fn tick(&mut self, cycles: u64) {
        self.cycles += cycles;

        if self.snes.superfx.rom_buffer.step(cycles) {
            self.wait_for_rom_access();
            let fx = &self.snes.superfx;
            let byte = self.gsu_bus_read(Address::new(fx.rombr, fx.r[14]));
            let fx = &mut self.snes.superfx;
            fx.rom_buffer.value = byte;
            fx.sfr.set_rom_busy(false);
        }

        if self.snes.superfx.ram_buffer.step(cycles) {
            self.wait_for_ram_access();
            let buffer = &self.snes.superfx.ram_buffer;
            let (addr, value) = (buffer.addr, buffer.value);
            self.ram_write(addr, value);
        }
    }

    fn wait_rom_op(&mut self) {
        let delay = self.snes.superfx.rom_buffer.delay;
        if delay > 0 {
            self.tick(delay as u64);
        }
    }

    fn wait_ram_op(&mut self) {
        let delay = self.snes.superfx.ram_buffer.delay;
        if delay > 0 {
            self.tick(delay as u64);
        }
    }

    fn wait_for_rom_access(&mut self) {
        if !self.snes.superfx.scmr.rom_to_gsu() {
            self.snes.superfx.wait_rom = true;
        }
    }

    fn wait_for_ram_access(&mut self) {
        if !self.snes.superfx.scmr.ram_to_gsu() {
            self.snes.superfx.wait_ram = true;
        }
    }

    fn read_rom_buffer(&mut self) -> u8 {
        self.wait_rom_op();
        self.snes.superfx.rom_buffer.value
    }

    fn ram_buffer_read(&mut self, addr: u16) -> u8 {
        self.wait_ram_op();
        self.wait_for_ram_access();
        self.ram_read(addr)
    }

    fn queue_ram_write(&mut self, addr: u16, value: u8) {
        self.wait_ram_op();

        let delay = self.bus_cost() as u8;
        self.snes.superfx.ram_buffer.queue(addr, value, delay);
    }

    fn read_opcode(&mut self) -> u8 {
        let out = self.snes.superfx.pipeline;
        self.snes.superfx.pipeline = self.fetch_program_byte();
        out
    }

    fn read_operand(&mut self) -> u8 {
        let out = self.snes.superfx.pipeline;
        self.snes.superfx.r[15] = self.snes.superfx.r[15].wrapping_add(1);
        self.snes.superfx.pipeline = self.fetch_program_byte();
        out
    }

    fn fetch_program_byte(&mut self) -> u8 {
        let fx = &self.snes.superfx;
        let pc = fx.r[15];

        #[cfg(feature = "debug")]
        {
            self.snes.superfx.pipeline_addr = pc;
        }
        let fx = &self.snes.superfx;
        let cache_off = pc.wrapping_sub(fx.cbr);

        if (cache_off as usize) < 0x200 {
            if !self.snes.superfx.cache.line_valid(cache_off) {
                self.fill_cache_line(cache_off & 0xFFF0);
            }
            self.tick(self.cache_cost());
            self.snes.superfx.cache.data[cache_off as usize]
        } else {
            self.wait_for_program_bus();
            self.tick(self.bus_cost());
            let fx = &self.snes.superfx;
            self.gsu_bus_read(Address::new(fx.pbr, fx.r[15]))
        }
    }

    fn wait_for_program_bus(&mut self) {
        if self.snes.superfx.pbr <= 0x5F {
            self.wait_rom_op();
            self.wait_for_rom_access();
        } else {
            self.wait_ram_op();
            self.wait_for_ram_access();
        }
    }

    fn fill_cache_line(&mut self, dest: u16) {
        self.wait_for_program_bus();

        let fx = &self.snes.superfx;
        let base = ((fx.pbr as u32) << 16) + fx.cbr as u32 + dest as u32;

        for i in 0..16u32 {
            let byte = self.gsu_bus_read(Address::from_24bit(base + i));
            self.snes.superfx.cache.data[dest as usize + i as usize] = byte;
        }
        self.tick(16 * self.bus_cost());

        self.snes.superfx.cache.mark_line(dest);
    }

    fn gsu_bus_read(&self, addr: Address) -> u8 {
        use crate::mmio::AddressSpace;
        match super::map::gsu_space(&self.snes.mmu.cartridge, addr) {
            AddressSpace::Cart(region, index) => self.snes.mmu.cart_read(region, index),
            _ => {
                tracing::error!(addr = format!("{addr:?}"), "GSU read from unmapped bank");
                0
            }
        }
    }

    fn ram_index(&self, addr: u16) -> usize {
        ((self.snes.superfx.rambr as usize) << 16) | addr as usize
    }

    fn ram_read(&self, addr: u16) -> u8 {
        self.snes.mmu.cartridge.sram_read(self.ram_index(addr))
    }

    fn ram_write(&mut self, addr: u16, value: u8) {
        let index = self.ram_index(addr);
        self.snes.mmu.cartridge.sram_write(index, value);
    }

    fn sfr(&self) -> &super::regs::Sfr {
        &self.snes.superfx.sfr
    }

    fn src(&self) -> u16 {
        self.snes.superfx.r[self.snes.superfx.sreg as usize]
    }

    fn write_reg(&mut self, reg: u8, value: u16) {
        self.snes.superfx.r[reg as usize] = value;
        if reg == 14 {
            self.snes.superfx.trigger_rom_prefetch();
        } else if reg == 15 {
            self.r15_changed = true;
        }
    }

    fn write_dest(&mut self, value: u16) {
        self.write_reg(self.snes.superfx.dreg, value);
    }

    fn reset_prefixes(&mut self) {
        let fx = &mut self.snes.superfx;
        fx.sfr.set_with(false);
        fx.sfr.set_alt1(false);
        fx.sfr.set_alt2(false);
        fx.sreg = 0;
        fx.dreg = 0;
    }

    fn set_nz(&mut self, value: u16) {
        let sfr = &mut self.snes.superfx.sfr;
        sfr.set_zero(value == 0);
        sfr.set_sign(value & 0x8000 != 0);
    }

    fn set_dest_nz(&mut self, value: u16) {
        self.write_dest(value);
        self.set_nz(value);

        self.reset_prefixes();
    }

    fn set_dest_nz8(&mut self, value: u8) {
        self.write_dest(value as u16);
        let sfr = &mut self.snes.superfx.sfr;
        sfr.set_zero(value == 0);
        sfr.set_sign(value & 0x80 != 0);

        self.reset_prefixes();
    }

    fn alt(&mut self, alt1: bool, alt2: bool) {
        let sfr = &mut self.snes.superfx.sfr;
        sfr.set_with(false);

        if alt1 {
            sfr.set_alt1(true);
        }

        if alt2 {
            sfr.set_alt2(true);
        }
    }

    fn store(&mut self, reg: u8, word: bool) {
        let addr = self.snes.superfx.r[reg as usize];
        self.snes.superfx.ram_addr = addr;

        let value = self.src();
        self.queue_ram_write(addr, value as u8);
        if word {
            self.queue_ram_write(addr ^ 1, (value >> 8) as u8);
        }

        self.reset_prefixes();
    }

    fn load(&mut self, reg: u8, word: bool) {
        let addr = self.snes.superfx.r[reg as usize];
        self.snes.superfx.ram_addr = addr;

        let mut value = self.ram_buffer_read(addr) as u16;
        if word {
            value |= (self.ram_buffer_read(addr ^ 1) as u16) << 8;
        }
        self.write_dest(value);

        self.reset_prefixes();
    }

    fn ram_operand_addr(&mut self, short: bool) -> u16 {
        let addr = if short {
            (self.read_operand() as u16) << 1
        } else {
            let lsb = self.read_operand() as u16;
            lsb | (self.read_operand() as u16) << 8
        };
        self.snes.superfx.ram_addr = addr;
        addr
    }

    fn lm(&mut self, reg: u8, short: bool) {
        let addr = self.ram_operand_addr(short);
        let lsb = self.ram_buffer_read(addr) as u16;
        let msb = self.ram_buffer_read(addr ^ 1) as u16;
        self.write_reg(reg, (msb << 8) | lsb);

        self.reset_prefixes();
    }

    fn sm(&mut self, reg: u8, short: bool) {
        let addr = self.ram_operand_addr(short);
        let value = self.snes.superfx.r[reg as usize];
        self.queue_ram_write(addr, value as u8);
        self.queue_ram_write(addr ^ 1, (value >> 8) as u8);

        self.reset_prefixes();
    }

    fn add(&mut self, operand: u16, with_carry: bool) {
        let src = self.src();
        let mut result = src as u32 + operand as u32;
        if with_carry {
            result += self.sfr().carry() as u32;
        }

        let sfr = &mut self.snes.superfx.sfr;
        sfr.set_carry(result & 0x1_0000 != 0);
        sfr.set_overflow(!(src ^ operand) & (operand ^ result as u16) & 0x8000 != 0);
        self.set_dest_nz(result as u16);
    }

    fn sub(&mut self, operand: u16, with_borrow: bool, write_back: bool) {
        let src = self.src();
        let mut result = src as i32 - operand as i32;
        if with_borrow {
            result -= !self.sfr().carry() as i32;
        }

        let sfr = &mut self.snes.superfx.sfr;
        sfr.set_carry(result >= 0);
        sfr.set_overflow((src ^ operand) & (src ^ result as u16) & 0x8000 != 0);
        self.set_nz(result as u16);

        if write_back {
            self.write_dest(result as u16);
        }

        self.reset_prefixes();
    }

    fn logic(&mut self, operand: u16, op: Logic) {
        let src = self.src();
        let value = match op {
            Logic::And => src & operand,
            Logic::BitClear => src & !operand,
            Logic::Or => src | operand,
            Logic::Xor => src ^ operand,
        };
        self.set_dest_nz(value);
    }

    fn mult(&mut self, operand: u16, signed: bool) {
        let value = if signed {
            ((self.src() as u8 as i8 as i16).wrapping_mul(operand as u8 as i8 as i16)) as u16
        } else {
            (self.src() as u8 as u16).wrapping_mul(operand as u8 as u16)
        };
        self.set_dest_nz(value);

        self.tick(if self.snes.superfx.cfgr.fast_multiply() {
            1
        } else {
            2
        });
    }

    fn fmult_lmult(&mut self, keep_low: bool) {
        let result =
            (self.src() as i16 as i32).wrapping_mul(self.snes.superfx.r[6] as i16 as i32) as u32;

        if keep_low {
            self.snes.superfx.r[4] = result as u16;
        }

        let value = (result >> 16) as u16;
        self.snes.superfx.sfr.set_carry(result & 0x8000 != 0);
        self.set_dest_nz(value);

        let base = if self.snes.superfx.cfgr.fast_multiply() {
            3
        } else {
            7
        };
        self.tick(base * self.cache_cost());
    }

    fn asr(&mut self, div2: bool) {
        let src = self.src();
        let mut value = ((src as i16) >> 1) as u16;
        if div2 {
            value = value.wrapping_add(((src as u32 + 1) >> 16) as u16);
        }

        self.snes.superfx.sfr.set_carry(src & 1 != 0);
        self.set_dest_nz(value);
    }

    fn step_reg(&mut self, reg: u8, delta: u16) {
        let value = self.snes.superfx.r[reg as usize].wrapping_add(delta);
        self.write_reg(reg, value);
        self.set_nz(value);

        self.reset_prefixes();
    }

    fn pixel_read(&self, offset: u32) -> u8 {
        self.snes.mmu.cartridge.sram_read(offset as usize)
    }

    fn pixel_write(&mut self, offset: u32, value: u8) {
        self.snes.mmu.cartridge.sram_write(offset as usize, value);
    }

    fn draw_pixel(&mut self, x: u8, y: u8) {
        let fx = &self.snes.superfx;
        let bpp = fx.scmr.bpp();
        if !fx.por.transparent() && fx.colr.is_transparent(&fx.por, bpp) {
            return;
        }
        let color = fx.colr.dithered(&fx.por, bpp, x, y);

        if !self.snes.superfx.pixel_primary.covers(x, y) {
            self.flush_primary(x, y);
        }
        self.snes.superfx.pixel_primary.insert(x, color);
        if self.snes.superfx.pixel_primary.full() {
            self.flush_primary(x, y);
        }
    }

    fn flush_primary(&mut self, x: u8, y: u8) {
        let secondary = self.snes.superfx.pixel_secondary;
        self.write_pixel_cache(secondary);

        let fx = &mut self.snes.superfx;
        fx.pixel_secondary = fx.pixel_primary;
        fx.pixel_primary.restart(x, y);
    }

    fn write_pixel_cache(&mut self, cache: PixelCache) {
        if cache.valid == 0 {
            return;
        }

        let row = Screen::of(&self.snes.superfx).tile_row(cache.x, cache.y);
        for plane in row.planes() {
            let mut value = cache.plane_bits(plane.index);

            if !cache.full() {
                self.tick(self.bus_cost());
                value = (value & cache.valid) | (self.pixel_read(plane.addr) & !cache.valid);
            }

            self.tick(self.bus_cost());
            self.wait_for_ram_access();
            self.pixel_write(plane.addr, value);
        }
    }

    fn read_pixel(&mut self, x: u8, y: u8) -> u8 {
        let secondary = self.snes.superfx.pixel_secondary;
        self.write_pixel_cache(secondary);
        self.snes.superfx.pixel_secondary.valid = 0;

        let primary = self.snes.superfx.pixel_primary;
        self.write_pixel_cache(primary);
        self.snes.superfx.pixel_primary.valid = 0;

        let row = Screen::of(&self.snes.superfx).tile_row(x, y);
        let bit = bitmap::pixel_bit(x);

        let mut data = 0u8;
        for plane in row.planes() {
            data |= ((self.pixel_read(plane.addr) >> bit) & 1) << plane.index;
            self.tick(self.bus_cost());
        }
        data
    }
}

impl<M: MemoryMap> decoder::Ops for Interpreter<'_, M> {
    fn nop(&mut self, _inst: Instruction) {
        self.reset_prefixes();
    }

    fn cache(&mut self, _inst: Instruction) {
        let fx = &mut self.snes.superfx;
        if fx.cbr != fx.r[15] & 0xFFF0 {
            fx.cbr = fx.r[15] & 0xFFF0;
            fx.cache.invalidate();
        }

        self.reset_prefixes();
    }

    fn halt(&mut self, _kind: HaltKind, _inst: Instruction) {
        #[cfg(feature = "debug")]
        {
            let fx = &self.snes.superfx;
            self.snes.superfx.stopped_screen = Some((fx.scbr, fx.scmr.raw(), fx.por.raw()));
        }

        self.snes.superfx.sfr.set_irq(true);
        if !self.snes.superfx.cfgr.irq_masked() {
            self.snes.irq.set_cart_irq(true);
        }

        self.snes.superfx.pipeline = 0x01;
        self.snes.superfx.sfr.set_go(false);

        self.reset_prefixes();
    }

    fn branches(&mut self, kind: BranchesKind, _inst: Instruction) {
        use BranchesKind as B;

        let sfr = self.sfr();
        let take = match kind {
            B::Bra => true,
            B::Bge => sfr.sign() == sfr.overflow(),
            B::Blt => sfr.sign() != sfr.overflow(),
            B::Bne => !sfr.zero(),
            B::Beq => sfr.zero(),
            B::Bpl => !sfr.sign(),
            B::Bmi => sfr.sign(),
            B::Bcc => !sfr.carry(),
            B::Bcs => sfr.carry(),
            B::Bvc => !sfr.overflow(),
            B::Bvs => sfr.overflow(),
        };

        let offset = self.read_operand() as i8;

        if take {
            let target = self.snes.superfx.r[15].wrapping_add_signed(offset as i16);
            self.write_reg(15, target);
        }
    }

    fn prefix_alt(&mut self, kind: PrefixAltKind, _inst: Instruction) {
        match kind {
            PrefixAltKind::Alt1 => self.alt(true, false),
            PrefixAltKind::Alt2 => self.alt(false, true),
            PrefixAltKind::Alt3 => self.alt(true, true),
        }
    }

    fn prefix_reg(&mut self, kind: PrefixRegKind, inst: Instruction) {
        let n = inst.low_nibble();

        match kind {
            PrefixRegKind::To => self.snes.superfx.dreg = n,
            PrefixRegKind::From => self.snes.superfx.sreg = n,
            PrefixRegKind::With => {
                let fx = &mut self.snes.superfx;
                fx.sreg = n;
                fx.dreg = n;
                fx.sfr.set_with(true);
            }
        }
    }

    fn move_reg(&mut self, kind: MoveRegKind, inst: Instruction) {
        let n = inst.low_nibble();

        match kind {
            MoveRegKind::Move => {
                let value = self.src();
                self.write_reg(n, value);

                self.reset_prefixes();
            }
            MoveRegKind::Moves => {
                let value = self.snes.superfx.r[n as usize];
                self.snes.superfx.sfr.set_overflow(value & 0x80 != 0);
                self.set_dest_nz(value);
            }
        }
    }

    fn alu(&mut self, kind: AluKind, inst: Instruction) {
        use AluKind as A;

        let n = inst.low_nibble();
        let reg = self.snes.superfx.r[n as usize];
        let imm = n as u16;

        match kind {
            A::AddReg => self.add(reg, false),
            A::AddImm => self.add(imm, false),
            A::AdcReg => self.add(reg, true),
            A::AdcImm => self.add(imm, true),
            A::SubReg => self.sub(reg, false, true),
            A::SubImm => self.sub(imm, false, true),
            A::SbcReg => self.sub(reg, true, true),
            A::CmpReg => self.sub(reg, false, false),
            A::AndReg => self.logic(reg, Logic::And),
            A::AndImm => self.logic(imm, Logic::And),
            A::BicReg => self.logic(reg, Logic::BitClear),
            A::BicImm => self.logic(imm, Logic::BitClear),
            A::OrReg => self.logic(reg, Logic::Or),
            A::OrImm => self.logic(imm, Logic::Or),
            A::XorReg => self.logic(reg, Logic::Xor),
            A::XorImm => self.logic(imm, Logic::Xor),
            A::Not => {
                let value = !self.src();
                self.set_dest_nz(value);
            }
        }
    }

    fn muldiv(&mut self, kind: MuldivKind, inst: Instruction) {
        use MuldivKind as K;

        let n = inst.low_nibble();
        let reg = self.snes.superfx.r[n as usize];

        match kind {
            K::MultReg => self.mult(reg, true),
            K::MultImm => self.mult(n as u16, true),
            K::UmultReg => self.mult(reg, false),
            K::UmultImm => self.mult(n as u16, false),
            K::Fmult => self.fmult_lmult(false),
            K::Lmult => self.fmult_lmult(true),
            K::Div2 => self.asr(true),
        }
    }

    fn shift(&mut self, kind: ShiftKind, _inst: Instruction) {
        match kind {
            ShiftKind::Lsr => {
                let src = self.src();
                let value = src >> 1;

                self.snes.superfx.sfr.set_carry(src & 1 != 0);
                self.set_dest_nz(value);
            }
            ShiftKind::Rol => {
                let src = self.src();
                let value = (src << 1) | self.sfr().carry() as u16;

                self.snes.superfx.sfr.set_carry(src & 0x8000 != 0);
                self.set_dest_nz(value);
            }
            ShiftKind::Asr => self.asr(false),
            ShiftKind::Ror => {
                let src = self.src();
                let value = (src >> 1) | ((self.sfr().carry() as u16) << 15);

                self.snes.superfx.sfr.set_carry(src & 1 != 0);
                self.set_dest_nz(value);
            }
        }
    }

    fn reg_step(&mut self, kind: RegStepKind, inst: Instruction) {
        let n = inst.low_nibble();

        match kind {
            RegStepKind::Inc => self.step_reg(n, 1),
            RegStepKind::Dec => self.step_reg(n, u16::MAX),
        }
    }

    fn byte_ops(&mut self, kind: ByteOpsKind, _inst: Instruction) {
        match kind {
            ByteOpsKind::Swap => {
                let value = self.src().rotate_right(8);
                self.set_dest_nz(value);
            }
            ByteOpsKind::Sex => {
                let value = self.src() as u8 as i8 as i16 as u16;
                self.set_dest_nz(value);
            }
            ByteOpsKind::Lob => self.set_dest_nz8(self.src() as u8),
            ByteOpsKind::Hib => self.set_dest_nz8((self.src() >> 8) as u8),
            ByteOpsKind::Merge => {
                let fx = &self.snes.superfx;
                let value = (fx.r[7] & 0xFF00) | (fx.r[8] >> 8);
                self.write_dest(value);

                let sfr = &mut self.snes.superfx.sfr;
                sfr.set_carry(value & 0xE0E0 != 0);
                sfr.set_overflow(value & 0xC0C0 != 0);
                sfr.set_sign(value & 0x8080 != 0);
                sfr.set_zero(value & 0xF0F0 != 0);

                self.reset_prefixes();
            }
        }
    }

    fn load_ram(&mut self, kind: LoadRamKind, inst: Instruction) {
        let n = inst.low_nibble();

        match kind {
            LoadRamKind::Ldw => self.load(n, true),
            LoadRamKind::Ldb => self.load(n, false),
            LoadRamKind::Lm => self.lm(n, false),
            LoadRamKind::Lms => self.lm(n, true),
        }
    }

    fn store_ram(&mut self, kind: StoreRamKind, inst: Instruction) {
        let n = inst.low_nibble();

        match kind {
            StoreRamKind::Stw => self.store(n, true),
            StoreRamKind::Stb => self.store(n, false),
            StoreRamKind::Sm => self.sm(n, false),
            StoreRamKind::Sms => self.sm(n, true),
            StoreRamKind::Sbk => {
                let addr = self.snes.superfx.ram_addr;
                let value = self.src();
                self.queue_ram_write(addr, value as u8);
                self.queue_ram_write(addr ^ 1, (value >> 8) as u8);

                self.reset_prefixes();
            }
        }
    }

    fn load_imm(&mut self, kind: LoadImmKind, inst: Instruction) {
        let n = inst.low_nibble();

        match kind {
            LoadImmKind::Ibt => {
                let value = self.read_operand() as i8;
                self.write_reg(n, value as i16 as u16);

                self.reset_prefixes();
            }
            LoadImmKind::Iwt => {
                let lsb = self.read_operand() as u16;
                let msb = self.read_operand() as u16;
                self.write_reg(n, (msb << 8) | lsb);

                self.reset_prefixes();
            }
        }
    }

    fn rom_read(&mut self, kind: RomReadKind, _inst: Instruction) {
        let byte = self.read_rom_buffer();

        match kind {
            RomReadKind::Getb => self.write_dest(byte as u16),
            RomReadKind::Getbh => self.write_dest((self.src() & 0x00FF) | ((byte as u16) << 8)),
            RomReadKind::Getbl => self.write_dest((self.src() & 0xFF00) | byte as u16),
            RomReadKind::Getbs => self.write_dest(byte as i8 as i16 as u16),
            RomReadKind::Getc => {
                let fx = &mut self.snes.superfx;
                fx.colr = fx.colr.merge(&fx.por, byte);
            }
        }

        self.reset_prefixes();
    }

    fn bank(&mut self, kind: BankKind, _inst: Instruction) {
        match kind {
            BankKind::Ramb => {
                self.wait_ram_op();
                self.snes.superfx.rambr = (self.src() & 1) as u8;
            }
            BankKind::Romb => {
                self.wait_rom_op();
                self.snes.superfx.rombr = (self.src() & 0x7F) as u8;
            }
        }

        self.reset_prefixes();
    }

    fn bitmap(&mut self, kind: BitmapKind, _inst: Instruction) {
        match kind {
            BitmapKind::Plot => {
                let (x, y) = (self.snes.superfx.r[1] as u8, self.snes.superfx.r[2] as u8);
                self.draw_pixel(x, y);
                self.snes.superfx.r[1] = self.snes.superfx.r[1].wrapping_add(1);

                self.reset_prefixes();
            }
            BitmapKind::Rpix => {
                let (x, y) = (self.snes.superfx.r[1] as u8, self.snes.superfx.r[2] as u8);
                let value = self.read_pixel(x, y);
                self.set_dest_nz(value as u16);
            }
            BitmapKind::Color => {
                let value = self.src() as u8;
                let fx = &mut self.snes.superfx;
                fx.colr = fx.colr.merge(&fx.por, value);

                self.reset_prefixes();
            }
            BitmapKind::Cmode => {
                self.snes.superfx.por = super::regs::Por::from_raw((self.src() & 0x1F) as u8);

                self.reset_prefixes();
            }
        }
    }

    fn jump(&mut self, kind: JumpKind, inst: Instruction) {
        use JumpKind as J;

        let n = inst.low_nibble();

        match kind {
            J::JmpR8 | J::JmpR9 | J::JmpR10 | J::JmpR11 | J::JmpR12 | J::JmpR13 => {
                self.write_reg(15, self.snes.superfx.r[n as usize]);

                self.reset_prefixes();
            }
            J::LjmpR8 | J::LjmpR9 | J::LjmpR10 | J::LjmpR11 | J::LjmpR12 | J::LjmpR13 => {
                self.snes.superfx.pbr = (self.snes.superfx.r[n as usize] & 0x7F) as u8;

                let target = self.src();
                self.write_reg(15, target);
                self.snes.superfx.cbr = target & 0xFFF0;
                self.snes.superfx.cache.invalidate();

                self.reset_prefixes();
            }
        }
    }

    fn call(&mut self, _kind: CallKind, inst: Instruction) {
        let n = inst.low_nibble();
        let fx = &mut self.snes.superfx;
        fx.r[11] = fx.r[15].wrapping_add(n as u16);

        self.reset_prefixes();
    }

    fn loop_ctl(&mut self, _kind: LoopCtlKind, _inst: Instruction) {
        let fx = &mut self.snes.superfx;
        fx.r[12] = fx.r[12].wrapping_sub(1);

        let value = fx.r[12];
        self.set_nz(value);

        if value != 0 {
            self.write_reg(15, self.snes.superfx.r[13]);
        }

        self.reset_prefixes();
    }

    fn on_invalid(&mut self, inst: Instruction) {
        panic!("Invalid GSU instruction: {:#04X}", inst.0);
    }
}

enum Logic {
    And,
    BitClear,
    Or,
    Xor,
}
