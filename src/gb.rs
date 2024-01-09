use crate::cpu::Cpu;
use crate::display::Display;
use anyhow::Result;
use winit::event_loop::EventLoop;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;

pub struct Gameboy {
    cpu: Cpu,
    display: Display<WIDTH, HEIGHT>,
}

impl Gameboy {
    pub fn new(bootrom: [u8; 0x100], cartridge: Vec<u8>) -> Self {
        let event_loop = EventLoop::new().unwrap();
        let display = Display::new(event_loop);
        let cpu = Cpu::new(bootrom, cartridge);
        Self { cpu, display }
    }

    pub fn run(self) -> Result<()> {
        self.display.run(
            self.cpu,
            |cpu| cpu.run_frame(),
            |ppu, pixels| ppu.render(pixels),
        )?;
        Ok(())
    }
}
