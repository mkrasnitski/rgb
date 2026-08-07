use anyhow::Result;
use std::fs::{self, File};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

use crate::apu::Apu;
use crate::bus::Cartridge;
use crate::config::{Args, Config};
use crate::cpu::Cpu;
use crate::debug::Debugger;
use crate::display::{Display, DisplayEvent};
use crate::hotkeys::Hotkey;

pub struct Gameboy {
    cpu: Cpu,
    display: Option<Display>,
    debugger: Option<Debugger>,
}

impl Gameboy {
    pub fn new(args: Args, config: Config) -> Result<Self> {
        let bootrom = if args.skip_bootrom {
            None
        } else {
            let default_path = if args.dmg_compat {
                "dmg_boot.bin"
            } else {
                "cgb_boot.bin"
            };
            Some(fs::read(config.bootrom.as_deref().unwrap_or(default_path))?)
        };
        let mut cartridge = Cartridge::new(&args.cartridge, &config.saves_dir)?;
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
        let apu = Apu::new(
            config.audio_volume,
            args.disable_audio || args.disable_video,
            !args.uncap_framerate,
        );
        let cpu = Cpu::new(bootrom, cartridge, apu, logfile);
        let display = if args.disable_video {
            None
        } else {
            Some(Display::new(
                config.keymap(),
                config.scale,
                !args.uncap_framerate,
            ))
        };
        Ok(Self {
            cpu,
            display,
            debugger: args.debug.map(Debugger::new).transpose()?,
        })
    }

    pub fn run(mut self) -> Result<()> {
        if self.display.is_some() {
            EventLoop::new()?.run_app(&mut self)?;
        } else {
            loop {
                self.cpu.run_frame(&mut self.debugger)?;
                if let Some(debugger) = &self.debugger
                    && debugger.quit
                {
                    break;
                }
            }
        }
        Ok(())
    }
}

impl ApplicationHandler for Gameboy {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(display) = &mut self.display
            && let Err(e) = display.reinit_surface(event_loop)
        {
            println!("{e:?}");
            display.quit(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(display) = &mut self.display
            && let Some(display_event) = display.process_event(&event)
        {
            match display_event {
                DisplayEvent::RedrawRequested => {
                    if let Err(e) = display.draw_frame(&mut self.cpu, &mut self.debugger) {
                        println!("{e:?}");
                        display.quit(event_loop);
                    }
                    if let Some(debugger) = &self.debugger
                        && debugger.quit
                    {
                        display.quit(event_loop);
                    }
                }
                DisplayEvent::Hotkey((hotkey, pressed)) => match hotkey {
                    Hotkey::Joypad(button) => {
                        self.cpu.joypad_mut().update_button(button, pressed);
                    }
                    Hotkey::ToggleFrameLimiter => {
                        if pressed {
                            display.toggle_frame_limiter();
                            self.cpu.toggle_frame_limiter();
                        }
                    }
                    Hotkey::Screenshot => {
                        if pressed {
                            if let Err(e) = self.cpu.ppu_mut().screenshot("a.png") {
                                println!("{e:?}");
                            }
                        }
                    }
                },
                DisplayEvent::Quit => {
                    if let Err(e) = self.cpu.save_external_ram() {
                        println!("Failed to save: {e:?}");
                    }
                    display.quit(event_loop);
                }
            }
        }
    }
}
