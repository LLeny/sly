-- Super Mario World 2: Yoshi's Island (USA)
--
-- Features:
--   * logs every regular coin pickup
--   * hold B for an unlimited "jump"
--   * keeps Baby Mario on Yoshi when enemies hit
--   * hold Select+Start to remove the hooks and restore the ROM patches

local WRAM = "wram"
local SRAM = "sram"
local ROM = "rom"

local GAME_MODE = 0x0118
local COINS = 0x037B
local CONTROLLER_1_DATA_2 = 0x093D
local PREVIOUS_CONTROLLER_1_DATA_2 = 0x0945

local YOSHI_STATE = 0x00AC
local YOSHI_JUMP_STATE = 0x00C0

local BUTTON_B = 0x80
local BUTTON_SELECT = 0x20
local BUTTON_START = 0x10

local baby_patches = {
  { offset = 0x01A8D7, original = 0x1F, patched = 0x00 },
  { offset = 0x034FB4, original = 0xD0, patched = 0x80 },
}

local function has_bit(value, mask)
  return value % (mask * 2) >= mask
end

local function install_patch(patch)
  local value = sly.read8(patch.offset, ROM)
  if value == patch.patched then
    return true
  end
  if value ~= patch.original then
    sly.error(string.format(
      "Baby Mario patch mismatch at ROM %s: expected %02X, found %02X",
      sly.hex(patch.offset, 6), patch.original, value))
    return false
  end

  sly.write8(patch.offset, patch.patched, ROM)
  return true
end

local function restore_patch(patch)
  if sly.read8(patch.offset, ROM) == patch.patched then
    sly.write8(patch.offset, patch.original, ROM)
  end
end

local installed = 0
for _, patch in ipairs(baby_patches) do
  if install_patch(patch) then
    installed = installed + 1
  end
end

if installed ~= #baby_patches then
  for _, patch in ipairs(baby_patches) do
    restore_patch(patch)
  end
  sly.error("Incompatible ROM!!!")
else
  sly.log("Baby Mario is safe now :)")
end

local last_coins = sly.read16(COINS, WRAM)

sly.on_write(COINS, COINS + 1, function(_, _, pc)
  local coins = sly.read16(COINS, WRAM)
  if coins > last_coins then
    sly.log(string.format(
      "Coin picked up: %d -> %d (+%d), PC=%s",
      last_coins, coins, coins - last_coins, sly.hex(pc, 4)))
  end
  last_coins = coins
end, { region = WRAM })

local function disable()
  for _, patch in ipairs(baby_patches) do
    restore_patch(patch)
  end
  sly.clear_hooks()
  sly.status("Patches disabled")
  sly.log("Patches restored")
end

sly.on_frame(function(frame)
  local controller_2 = sly.read8(CONTROLLER_1_DATA_2, WRAM)

  if has_bit(controller_2, BUTTON_SELECT)
      and has_bit(controller_2, BUTTON_START) then
    disable()
    return
  end

  local mode = "ready"
  local gameplay = sly.read16(GAME_MODE, WRAM) == 0x000F
  local yoshi_active = sly.read16(YOSHI_STATE, SRAM) == 0x0000
  local jump_state = sly.read16(YOSHI_JUMP_STATE, SRAM)
  local airborne = jump_state >= 0x0006 and jump_state <= 0x0008
  local holding_b = has_bit(controller_2, BUTTON_B)

  if gameplay and yoshi_active and airborne and holding_b then
    local previous = sly.read8(PREVIOUS_CONTROLLER_1_DATA_2, WRAM)
    if has_bit(previous, BUTTON_B) then
      sly.write8(PREVIOUS_CONTROLLER_1_DATA_2, previous - BUTTON_B, WRAM)
    end

    mode = "infinite hover"
  end

  if frame % 6 == 0 then
    sly.status(string.format(
      "Yoshi's Island | coins %d | %s",
      sly.read16(COINS, WRAM), mode))
  end
end)
