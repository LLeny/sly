use super::*;

impl SnesDebug {
    pub(super) fn region_byte(&self, region: &str, addr: u32) -> u8 {
        match region {
            "wram" => self.core.mmu.wram.get(addr as usize).copied().unwrap_or(0),
            "iram" => self.core.sa1.iram.get(addr as usize).copied().unwrap_or(0),
            "sram" => self
                .core
                .mmu
                .cartridge
                .sram
                .get(addr as usize)
                .copied()
                .unwrap_or(0),
            "rom" => self
                .core
                .mmu
                .cartridge
                .rom
                .get(addr as usize)
                .copied()
                .unwrap_or(0),
            "bus" => self.core.peek8(Address::from_24bit(addr & 0x00FF_FFFF)),
            "spc" => self.core.apu.peek(addr as u16),
            "gsu" => superfx::bus_peek(&self.core, (addr >> 16) as u8 & 0x7F, addr as u16),
            "gsucache" => self.core.superfx.cache.data[addr as usize & 0x1FF],
            _ => self.core.peek8(Address::new(0x00, addr as u16)),
        }
    }

    pub(super) fn region_key(&self, region: &str, addr: u32) -> Option<(MemKind, u32)> {
        match region {
            "wram" => Some((MemKind::Wram, addr)),
            "iram" => Some((MemKind::Iram, addr)),
            "sram" => Some((MemKind::Sram, addr)),
            "rom" => Some((MemKind::Rom, addr)),
            "bus" => self.core.classify(Address::from_24bit(addr & 0x00FF_FFFF)),
            "spc" => Some((MemKind::Spc, addr)),
            "gsu" | "gsucache" => None,
            _ => Some((MemKind::Io, addr)),
        }
    }

    pub(super) fn regions(&self) -> Vec<MemoryRegion> {
        let mut regions = vec![
            MemoryRegion::new("wram", "WRAM", 0, WRAM_SIZE),
            MemoryRegion::new("rom", "ROM", 0, self.core.mmu.cartridge.rom.len() as u32),
            MemoryRegion::new("ppu", "PPU/APU", PPU_BASE, PPU_SIZE),
            MemoryRegion::new("cpu", "CPU/DMA", CPU_BASE, CPU_SIZE),
            MemoryRegion::new("bus", "CPU Bus", 0, BUS_SIZE),
            MemoryRegion::new("spc", "SPC RAM", 0, 0x1_0000),
        ];
        if !self.core.mmu.cartridge.sram.is_empty() {
            let name = if self.has_gsu() {
                "Game Pak RAM"
            } else {
                "Cartridge SRAM"
            };
            regions.insert(
                1,
                MemoryRegion::new("sram", name, 0, self.core.mmu.cartridge.sram.len() as u32),
            );
        }
        if self.has_sa1() {
            let iram_size = u32::try_from(crate::sa1::IRAM_SIZE)
                .expect("SA-1 I-RAM size must fit in the debugger address space");
            regions.insert(1, MemoryRegion::new("iram", "SA-1 I-RAM", 0, iram_size));
        }
        if self.has_gsu() {
            regions.push(MemoryRegion::new("gsu", "GSU Bus", 0, 0x80_0000));
            regions.push(MemoryRegion::new("gsucache", "GSU Cache", 0, 0x200));
        }
        regions
    }

    pub(super) fn read(&self, region: &str, addr: u32, len: usize) -> Vec<u8> {
        fn window(data: &[u8], addr: u32, len: usize) -> Vec<u8> {
            let mut out = vec![0u8; len];

            let start = (addr as usize).min(data.len());
            let end = start.saturating_add(len).min(data.len());
            out[..end - start].copy_from_slice(&data[start..end]);

            out
        }

        match region {
            "wram" => window(&self.core.mmu.wram, addr, len),
            "iram" => window(&self.core.sa1.iram, addr, len),
            "sram" => window(&self.core.mmu.cartridge.sram, addr, len),
            "rom" => window(&self.core.mmu.cartridge.rom, addr, len),
            _ => (0..len as u32)
                .map(|i| self.region_byte(region, addr.wrapping_add(i)))
                .collect(),
        }
    }

    pub(super) fn write(&mut self, region: &str, addr: u32, bytes: &[u8]) -> usize {
        fn patch(data: &mut [u8], addr: u32, bytes: &[u8]) -> usize {
            let start = (addr as usize).min(data.len());
            let end = start.saturating_add(bytes.len()).min(data.len());
            data[start..end].copy_from_slice(&bytes[..end - start]);
            end - start
        }

        match region {
            "wram" => patch(&mut self.core.mmu.wram, addr, bytes),
            "iram" => patch(&mut self.core.sa1.iram, addr, bytes),
            "sram" => patch(&mut self.core.mmu.cartridge.sram, addr, bytes),
            "rom" => patch(&mut self.core.mmu.cartridge.rom, addr, bytes),
            "spc" => {
                for (ix, byte) in bytes.iter().enumerate() {
                    self.core
                        .apu
                        .write(addr.wrapping_add(ix as u32) as u16, *byte);
                }
                bytes.len()
            }
            "gsu" | "gsucache" => 0,
            "bus" => {
                for (ix, byte) in bytes.iter().enumerate() {
                    let addr = addr.wrapping_add(ix as u32) & 0x00FF_FFFF;
                    self.core.write8(Address::from_24bit(addr), *byte);
                }
                bytes.len()
            }
            _ => {
                for (ix, byte) in bytes.iter().enumerate() {
                    let addr = addr.wrapping_add(ix as u32) as u16;
                    self.core.write8(Address::new(0x00, addr), *byte);
                }
                bytes.len()
            }
        }
    }

    pub(super) fn format_addr(&self, region: &str, addr: u32) -> String {
        match region {
            "wram" => format!("{:02X}:{:04X}", 0x7E + (addr >> 16), addr & 0xFFFF),
            "iram" => format!("{addr:03X}"),
            "sram" => format!("{addr:06X}"),
            "rom" => format!("{addr:06X}"),
            "bus" => format!("{:02X}:{:04X}", (addr >> 16) & 0xFF, addr & 0xFFFF),
            "spc" => format!("{:04X}", addr & 0xFFFF),
            "gsu" => format!("{:02X}:{:04X}", (addr >> 16) & 0x7F, addr & 0xFFFF),
            "gsucache" => format!("{:03X}", addr & 0x1FF),
            _ => format!("00:{:04X}", addr & 0xFFFF),
        }
    }

    pub(super) fn highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight> {
        self.capture_frames
            .store(CAPTURE_FRAMES, std::sync::atomic::Ordering::Relaxed);

        (0..len as u32)
            .map(|i| match self.region_key(region, addr.wrapping_add(i)) {
                Some(key) if self.last_access.writes.contains(&key) => Highlight::Written,
                Some(key) if self.last_access.reads.contains(&key) => Highlight::Read,
                _ => Highlight::None,
            })
            .collect()
    }

    pub(super) fn annotation(&self, addr: u32) -> String {
        crate::mmio::symbol((addr & 0xFFFF) as u16)
            .map(str::to_string)
            .unwrap_or_default()
    }

    pub(super) fn latest(&self) -> Option<(Arc<str>, u32)> {
        self.track_frames
            .store(TRACK_FRAMES, std::sync::atomic::Ordering::Relaxed);

        let (kind, offset) = self.last_access.last?;
        let region = match kind {
            MemKind::Wram => "wram",
            MemKind::Iram => "iram",
            MemKind::Sram => "sram",
            MemKind::Rom => "rom",
            MemKind::Io if (PPU_BASE..PPU_BASE + PPU_SIZE).contains(&offset) => "ppu",
            MemKind::Io if (CPU_BASE..CPU_BASE + CPU_SIZE).contains(&offset) => "cpu",
            MemKind::Io => "bus",
            MemKind::Spc => "spc",
        };
        Some((region.into(), offset))
    }
}
