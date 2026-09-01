mod apu;
mod bus;
mod config;
mod cpu;
mod debug;
mod display;
mod gb;
mod hotkeys;
mod ppu;
mod utils;

use config::{Args, Config};
use gb::Gameboy;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::new(args.config.as_ref())?;
    Gameboy::new(args, config)?.run()
}
