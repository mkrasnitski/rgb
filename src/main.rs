mod bus;
mod cpu;
mod utils;

use anyhow::Result;
use cpu::Cpu;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let bootrom = std::fs::read("dmg_boot.bin")?
        .try_into()
        .expect("Bootrom not 0x100 in length");
    let cartridge = std::fs::read(&args[1])?;
    let cpu = Cpu::new(bootrom, cartridge);
    cpu.run()?;
    Ok(())
}
