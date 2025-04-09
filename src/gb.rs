use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

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
        let display = Display::new(config.keymap(), args.scale);
        let bootrom = std::fs::read(config.bootrom)?
            .try_into()
            .expect("Bootrom not 0x100 in length");
        let mut cartridge = Cartridge::new(args.cartridge, config.saves_dir)?;
        cartridge.load_external_ram()?;
        let cpu = Cpu::new(bootrom, cartridge, args.skip_bootrom, args.debug);
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
