mod bus;
mod config;
mod cpu;
mod display;
mod gb;
mod hotkeys;
mod ppu;
mod utils;

use config::{Args, Config};
use gb::Gameboy;
use winit::event_loop::EventLoop;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::new("config.toml".as_ref())?;
    let mut gb = Gameboy::new(args, config)?;
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut gb)?;
    Ok(())
}
