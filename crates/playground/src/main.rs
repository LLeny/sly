fn main() {
    use core_model::Core;
    use snes::Snes;

    tracing_subscriber::fmt::init();

    let path = std::env::args()
        .nth(1)
        .expect("Usage: playground <path-to-rom>");
    let addr = std::env::args()
        .nth(2)
        .and_then(|s| u16::from_str_radix(&s, 16).ok());

    let buffer = std::fs::read(&path).expect("Failed to read ROM");

    let mut snes = Snes::new();
    snes.load_cartridge(core_model::RomFile::new().data(buffer).filename(&path))
        .expect("Failed to load cartridge");

    let info = <snes::Cartridge as core_model::Cartridge>::info(&snes.mmu.cartridge)
        .expect("Failed to parse cartridge header");
    println!("Loaded ROM: {}", info.title);

    let mut data = snes.mmu.cartridge.data().to_vec();
    let header = snes::cartridge_header::CartridgeHeader::parse(&mut data)
        .expect("Failed to parse cartridge header");
    println!("Memory map: {:?}", header.map_mode());

    loop {
        if let Some(addr) = addr
            && snes.cpu.pc == addr
        {
            break;
        }

        let disasm = snes.disassemble_next();
        println!(
            "{:02X}:{:04X}: {}",
            snes.cpu.program_bank, snes.cpu.pc, disasm
        );
        snes.step();
    }
}
