use anyhow::Result;
use std::fs::File;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::apu::Apu;
use crate::bus::Cartridge;
use crate::config::{Args, Config};
use crate::cpu::Cpu;
use crate::display::{Display, DisplayEvent};
use crate::hotkeys::Hotkey;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;

pub struct Gameboy {
    cpu: Cpu,
    display: Display<WIDTH, HEIGHT>,
}

impl Gameboy {
    pub fn new(args: Args, config: Config) -> Result<Self> {
        let display = Display::new(config.keymap(), config.scale);
        let bootrom = if args.skip_bootrom {
            None
        } else {
            Some(
                std::fs::read(config.bootrom)?
                    .try_into()
                    .expect("Bootrom not 0x100 in length"),
            )
        };
        let mut cartridge = Cartridge::new(args.cartridge, config.saves_dir)?;
        cartridge.load_external_ram()?;
        let logfile = args
            .logfile
            .map(|path| {
                if path.display().to_string() == "-" {
                    Ok(Box::new(std::io::stdout()) as Box<_>)
                } else {
                    File::create(path).map(|file| Box::new(file) as Box<_>)
                }
            })
            .transpose()?;
        let apu = Apu::new(config.audio_volume, args.disable_audio);
        let cpu = Cpu::new(bootrom, cartridge, apu, logfile);
        Ok(Self { cpu, display })
    }
}

impl ApplicationHandler for Gameboy {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = self.display.reinit_surface(event_loop) {
            println!("{e:?}");
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(display_event) = self.display.process_event(&event) {
            match display_event {
                DisplayEvent::RedrawRequested => {
                    if let Err(e) = self.display.draw_frame(&mut self.cpu) {
                        println!("{e:?}");
                        event_loop.exit();
                    }
                }
                DisplayEvent::Hotkey((hotkey, pressed)) => match hotkey {
                    Hotkey::Joypad(button) => {
                        self.cpu.joypad_mut().update_button(button, pressed);
                    }
                    Hotkey::ToggleFrameLimiter => {
                        if pressed {
                            self.display.toggle_frame_limiter();
                            self.cpu.toggle_frame_limiter();
                        }
                    }
                },
                DisplayEvent::Quit => {
                    if let Err(e) = self.cpu.save_external_ram() {
                        println!("Failed to save: {e:?}");
                    }
                    event_loop.exit()
                }
            }
        }
    }
}
