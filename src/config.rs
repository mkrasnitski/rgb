use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::hotkeys::{KeyMap, Keybindings};

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
pub struct Args {
    #[arg(id = "rom-path", hide = true)]
    pub cartridge: PathBuf,

    #[arg(long)]
    pub skip_bootrom: bool,

    #[arg(short, long, default_value = "config.toml", help = "Config file")]
    pub config: PathBuf,

    #[arg(short, long, help = "Enable debug logs")]
    pub logfile: Option<PathBuf>,

    #[arg(short, long, help = "Scale factor", default_value = "3")]
    pub scale: u32,
}

#[derive(Deserialize)]
pub struct Config {
    pub bootrom: String,
    pub saves_dir: PathBuf,
    #[serde(rename = "volume")]
    pub audio_volume: f32,
    #[serde(rename = "hotkeys")]
    keybindings: Keybindings,
}

impl Config {
    pub fn new(path: &Path) -> Result<Self> {
        let config = match File::open(path) {
            Ok(mut file) => {
                let mut toml = String::new();
                file.read_to_string(&mut toml)?;
                toml::from_str(&toml)?
            }
            Err(e) => {
                println!("{}: {e}", path.display());
                println!("Using default config.");
                Config {
                    bootrom: "dmg_boot.bin".to_string(),
                    saves_dir: "saves".into(),
                    audio_volume: 100.0,
                    keybindings: Keybindings::default(),
                }
            }
        };
        Ok(config)
    }

    pub fn keymap(&self) -> KeyMap {
        KeyMap::new(&self.keybindings)
    }
}
