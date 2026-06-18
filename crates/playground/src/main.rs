fn main() {
    use snes::Cartridge;

    let path = std::env::args()
        .nth(1)
        .expect("Usage: playground <path-to-rom>");
    let buffer = std::fs::read(&path).expect("Failed to read ROM");

    let cart = Cartridge::new(buffer).expect("Failed to create cartridge");
    let title = cart.title();
    println!("Loaded ROM: {}", title);
}
