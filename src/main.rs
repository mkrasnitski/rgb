mod bus;
mod cpu;
mod display;
mod gb;
mod ppu;
mod utils;

use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let bootrom = std::fs::read("dmg_boot.bin")?
        .try_into()
        .expect("Bootrom not 0x100 in length");
    let cartridge = std::fs::read(&args[1])?;
    let gb = crate::gb::Gameboy::new(bootrom, cartridge);
    gb.run()
}
