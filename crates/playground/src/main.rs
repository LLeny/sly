fn main() {
    use snes::{Cartridge, Snes};

    tracing_subscriber::fmt::init();

    let path = std::env::args()
        .nth(1)
        .expect("Usage: playground <path-to-rom>");
    let count = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let buffer = std::fs::read(&path).expect("Failed to read ROM");

    let cart = Cartridge::new(buffer).expect("Failed to create cartridge");
    let title = cart.title();
    println!("Loaded ROM: {}", title);

    let memory_map = cart.memory_map().expect("Failed to get memory map");
    println!("Memory map: {:?}", memory_map);

    let mut snes = Snes::new(cart);

    for _ in 0..count {
        if snes.cpu.program_bank == 0x00 && snes.cpu.pc == 0x8085 {
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
