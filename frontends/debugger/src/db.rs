use artwork::{Header, System};

include!(concat!(env!("OUT_DIR"), "/titles.rs"));

pub fn title(system: System, rom: &[u8]) -> Option<&'static str> {
    let info = system.info();
    let table = titles(info.libretro_name)?;
    let crc = self::rom_crc32(info.header, rom);
    table
        .binary_search_by_key(&crc, |&(crc, _)| crc)
        .ok()
        .map(|i| table[i].1)
}

fn rom_crc32(header: Header, rom: &[u8]) -> u32 {
    crc32fast::hash(header.strip(rom))
}
