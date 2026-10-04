# Sly

Sly is a cross-platform, multi-system emulator for players, homebrew developers and emulator developers alike. It combines a beautiful debugger with a unique take on the classic emulation experience!

[Official website](https://sly.layle.dev/) · [Try the debugger in your browser](https://sly.layle.dev/app)

<p>
  <img src="assets/screenshots/debugger.png" width="49%" alt="Sly Debugger">
  <img src="assets/screenshots/launcher.png" width="49%" alt="Sly Launcher">
</p>

Sly has three main desktop frontends, alongside its mobile apps:

* **Sly Launcher**: A 3D game library built around box art and cartridges.
* **Sly Classic**: A minimal Qt interface for browsing and playing games.
* **Sly Debugger**: A beautiful debugger packed with features.

Supported platforms:

* Windows, macOS and Linux
* Web browsers via WebGPU (debugger only so far)
* iOS, with Android planned

## Download

Get the [latest release](https://github.com/ioncodes/sly/releases/latest):

* **Windows, macOS and Linux**: each frontend comes as its own archive: `sly-launcher`, `sly-classic` and `sly-debugger`.
* **iOS**: `sly-ios-arm64-unsigned.ipa`. It's unsigned, so you need to sideload it (for example with AltStore or SideStore).

For the newest changes, try the [nightly build](https://github.com/ioncodes/sly/releases/tag/nightly). It's rebuilt from the `dev` branch once a day and may be unstable.

## Compatibility

Color means playable or work-in-progress. Click on a logo to see the respective compatibility database.

<table>
  <tr>
    <td><a href="https://emu.layle.dev/sly-snes"><img src="assets/systems/snes.svg" width="240" alt="Super Nintendo" title="Super Nintendo screenshot database"></a></td>
    <td><img src="assets/systems/gba.svg" width="240" alt="Game Boy Advance"></td>
    <td><img src="assets/systems/gb.svg" width="240" alt="Game Boy and Game Boy Color"></td>
    <td><img src="assets/systems/gg.svg" width="240" alt="Game Gear"></td>
  </tr>
  <tr>
    <td><img src="assets/systems/sms.svg" width="240" alt="Master System"></td>
    <td><img src="assets/systems/psx.svg" width="240" alt="PlayStation"></td>
    <td><img src="assets/systems/gc.svg" width="240" alt="GameCube"></td>
    <td><img src="assets/systems/wii.svg" width="240" alt="Wii"></td>
  </tr>
  <tr>
    <td><img src="assets/systems/genesis.svg" width="240" alt="Sega Genesis"></td>
    <td><img src="assets/systems/dreamcast.svg" width="240" alt="Dreamcast"></td>
    <td><img src="assets/systems/nds.svg" width="240" alt="Nintendo DS"></td>
    <td><img src="assets/systems/3ds.svg" width="240" alt="Nintendo 3DS"></td>
  </tr>
</table>

## Features

**Sly Launcher**
* 3D boxes and cartridges
* Row, column and ribbon shelf layouts
* Flippable box art
* Automatic artwork downloads
* Regional packaging and models, with per-game release overrides
* Per-game cartridge finishes
* Gamepad navigation
* Remappable controls
* Save states
* Quality presets, text size, reduced motion and integer scaling
* Fullscreen

**Sly Classic**
* Sortable game list with banners
* Quick save slot plus ten save state slots
* 1x to 4x scaling and fullscreen
* Gamepad support
* Automatic artwork downloads

**Sly Debugger**
* Instruction and event stepping
* Linear and graph disassembly
* CPU registers and call stacks
* CPU and memory breakpoints
* Memory viewer
* MMIO register inspection
* DMA inspection
* Tile, tilemap and background viewers
* Sprite and palette viewers
* Various coprocessor workspaces (for example SA-1 and Super FX)
* Audio visualization
* Built-in Lua editor and scripting
* Save states
* Cartridge info with cover art
* Light and dark themes with various accent colors

## Usage

| System | File types     |
| ------ | -------------- |
| SNES   | `.sfc`, `.smc` |
| GBA    | `.gba`         |

How you load games depends on the frontend:

* **Sly Launcher**: add your ROM folders in settings, or pass a folder on the command line.
* **Sly Classic**: put your ROMs in `roms/{system}/` in the working directory.
* **iOS**: copy games into Sly's `roms/{system}/` folders using the Files app.
* **Sly Debugger**: pass the ROM path on the command line, or drag it into the window.

From the command line:

```sh
sly-launcher path/to/roms/
sly-debugger path/to/game.sfc
```

## Artwork

Box art and covers come from [art.layle.dev](https://art.layle.dev). This works without an account, but anonymous access has lower rate limits. For higher limits, log in at [art.layle.dev/account](https://art.layle.dev/account), generate an API key and paste it into the settings of any frontend:

* **Sly Launcher**: Settings -> Artwork key
* **Sly Classic**: File -> Settings
* **Sly Debugger**: the settings popover, under Artwork
* **iOS**: Settings -> Artwork

The key is stored in `settings.toml` in Sly's data folder (for example `%APPDATA%\sly` on Windows), which all desktop frontends share.

## Default Controls

These are Sly Launcher's defaults. All of them can be remapped in settings (gamepads are supported too).

Keys are matched by position, so the labels below are for a US QWERTY keyboard. On other layouts, press the key in the same place (for example, **Z** is **Y** on a German keyboard).

**Library**

| Action                  | Key               |
| ----------------------- | ----------------- |
| Browse                  | Arrow keys / WASD |
| Play                    | Enter / Space     |
| Flip box                | F                 |
| Change view             | Tab               |
| Search                  | /                 |
| Previous / next letter  | Q / E             |
| Console list            | C                 |
| Previous / next console | Page Up / Down    |
| Settings                | F2                |
| Fullscreen              | F11               |

**In game**

| Control           | Key               |
| ----------------- | ----------------- |
| D-pad             | Arrow keys        |
| B / A             | Z / X             |
| Y / X (SNES)      | A / S             |
| L / R             | Q / W             |
| Start / Select    | Enter / Backspace |
| Save / load state | F5 / F8           |
| Pause             | P                 |
| Back to library   | Esc               |
| Fullscreen        | F11               |

## Development Setup

See the [build workflow](.github/workflows/binaries.yml) for platform dependencies, including Linux audio and windowing libraries. Run everything from the repository root.

**Sly Launcher and Sly Debugger**

```sh
git submodule update --init --recursive
cargo run --locked -p modern-launcher -- roms/
cargo run --locked -p debugger -- path/to/game.sfc
```

This should work for most of the Rust crates.

**Sly Classic** also needs:

* Qt 6 (CI uses 6.11.1), including Multimedia, ShaderTools and private GUI headers
* CMake 4.3 or newer
* Ninja
* A C++23 compiler
* naga-cli 29.0.4 (`cargo install naga-cli --version 29.0.4 --locked`)
* SDL3 3.2 or newer for gamepads (or configure with `-DSLY_ENABLE_GAMEPAD=OFF`)

```sh
qt-cmake -S frontends/desktop-qt -B build/desktop-qt -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DSLY_CARGO_PROFILE=release
cmake --build build/desktop-qt
```

**iOS** needs a Mac with Xcode 26. Open `frontends/ios/Sly.xcodeproj`, choose your signing team and run the `Sly` scheme.

## License

[GPL-3.0](LICENSE) · [Third-party notices](licenses)

Sly is not affiliated with Nintendo or other console manufacturers. Game titles, artwork and trademarks belong to their respective owners.
