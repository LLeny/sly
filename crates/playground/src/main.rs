fn main() {
    use snes::{Cartridge, Snes};

    let path = std::env::args()
        .nth(1)
        .expect("Usage: playground <path-to-rom>");
    let buffer = std::fs::read(&path).expect("Failed to read ROM");

    let cart = Cartridge::new(buffer).expect("Failed to create cartridge");
    let title = cart.title();
    println!("Loaded ROM: {}", title);

    let memory_map = cart.memory_map().expect("Failed to get memory map");
    println!("Memory map: {:?}", memory_map);

    let snes = Snes::new(cart);
    let disasm = snes.disassemble_next();
    println!("{:04X}: {}", snes.cpu.pc, disasm);
}
