mod bus;
mod config;
mod cpu;
mod display;
mod gb;
mod hotkeys;
mod ppu;
mod utils;

use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let config = config::Config::new("config.toml".as_ref())?;
    let cartridge = std::fs::read(&args[1])?;
    let gb = gb::Gameboy::new(cartridge, config)?;
    gb.run()
}
