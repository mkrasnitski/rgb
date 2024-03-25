use crate::config::Config;
use crate::cpu::Cpu;
use crate::display::{Display, DisplayEvent};
use crate::hotkeys::Hotkey;
use anyhow::Result;
use winit::event_loop::EventLoop;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;

pub struct Gameboy {
    cpu: Cpu,
    display: Display<WIDTH, HEIGHT>,
    event_loop: EventLoop<()>,
}

impl Gameboy {
    pub fn new(cartridge: Vec<u8>, config: Config) -> Result<Self> {
        let event_loop = EventLoop::new().unwrap();
        let display = Display::new(&event_loop, config.keymap());
        let bootrom = std::fs::read(config.bootrom)?
            .try_into()
            .expect("Bootrom not 0x100 in length");
        let cpu = Cpu::new(bootrom, cartridge);
        Ok(Self {
            cpu,
            display,
            event_loop,
        })
    }

    pub fn run(mut self) -> Result<()> {
        Ok(self.event_loop.run(|event, elwt| {
            if let Some(display_event) = self.display.process_winit_events(&event) {
                match display_event {
                    DisplayEvent::RedrawRequested => {
                        if let Err(e) = self.display.draw_frame(&mut self.cpu) {
                            println!("{e:?}");
                            elwt.exit();
                        }
                    }
                    DisplayEvent::Hotkey((hotkey, pressed)) => match hotkey {
                        Hotkey::Joypad(button) => {
                            self.cpu.joypad_mut().update_button(button, pressed)
                        }
                        Hotkey::ToggleFrameLimiter => {
                            if pressed {
                                self.display.toggle_frame_limiter()
                            }
                        }
                    },
                    DisplayEvent::Quit => elwt.exit(),
                }
            }
        })?)
    }
}
