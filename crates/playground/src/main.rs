fn main() {
    use snes::{Cartridge, Snes};

    tracing_subscriber::fmt::init();

    let path = std::env::args()
        .nth(1)
        .expect("Usage: playground <path-to-rom>");
    let addr = std::env::args()
        .nth(2)
        .and_then(|s| u16::from_str_radix(&s, 16).ok());

    let buffer = std::fs::read(&path).expect("Failed to read ROM");

    let cart = Cartridge::new(buffer).expect("Failed to create cartridge");
    let title = cart.title();
    println!("Loaded ROM: {}", title);

    let memory_map = cart.memory_map().expect("Failed to get memory map");
    println!("Memory map: {:?}", memory_map);

    let mut snes = Snes::new(cart);

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
