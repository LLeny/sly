-- Regions: bus, wram, sram, rom, spc (see sly.regions()).
-- Events:  scanline, nmi, irq        (see sly.events()).

sly.log("regions: " .. sly.dump(sly.regions()))

sly.on_frame(function(frame)
  if frame % 60 == 0 then
    sly.status(string.format("frame %d  pc=%s  A=%s", frame,
      sly.hex(sly.pc(), 6), sly.hex(sly.reg("A"), 4)))
  end
end)

-- Fires at the start of vblank, with the scanline and the PC.
-- sly.on_event("nmi", function(line, detail, pc)
--   sly.log(string.format("nmi on line %d from %s", line, sly.hex(pc, 6)))
-- end)

-- Watch a WRAM byte (address is a bus address here).
-- sly.on_write(0x7E0010, function(addr, value, pc)
--   sly.log(string.format("%s <- %02X from %s", sly.hex(addr, 6), value, sly.hex(pc, 6)))
-- end)

-- Break into the debugger when the CPU reaches an address.
-- sly.on_exec(0x008000, function(addr, pc)
--   sly.log("reached " .. sly.hex(addr, 6))
--   sly.pause()
-- end)

-- Search WRAM for a byte pattern.
-- sly.log(sly.dump(sly.find({ 0x42, 0x00 }, { region = "wram", limit = 8 })))
