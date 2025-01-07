use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::cpu::Cpu;
use crate::hotkeys::{Hotkey, KeyMap};

use anyhow::Result;
use pixels::{Pixels, SurfaceTexture};
use spin_sleep_util::Interval;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event as WinitEvent, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window as WinitWindow, WindowBuilder},
};

const FRAMERATE: f64 = 4194304.0 / 70224.0;

pub enum DisplayEvent {
    Hotkey((Hotkey, bool)),
    RedrawRequested,
    Quit,
}

pub struct Display<const W: u32, const H: u32> {
    surface: Surface<W, H>,
    keymap: KeyMap,
    frame_limiter: FrameLimiter,
}

impl<const W: u32, const H: u32> Display<W, H> {
    pub fn new(event_loop: &EventLoop<()>, keymap: KeyMap, scale_factor: u32) -> Result<Self> {
        Ok(Self {
            surface: Surface::new(event_loop, scale_factor)?,
            keymap,
            frame_limiter: FrameLimiter {
                limit_framerate: true,
                frame_limiter: spin_sleep_util::interval(Duration::from_secs_f64(1.0 / FRAMERATE)),
                frame_time: None,
                instant: Instant::now(),
            },
        })
    }

    pub fn draw_frame(&mut self, cpu: &mut Cpu) -> Result<()> {
        self.frame_limiter.run(|| cpu.run_frame())?;
        cpu.ppu_mut().render(&mut self.surface.pixels)?;
        self.frame_limiter.tick();
        Ok(())
    }

    pub fn process_event(&mut self, event: &WinitEvent<()>) -> Option<DisplayEvent> {
        let WinitEvent::WindowEvent { event, .. } = event else {
            return None;
        };
        match event {
            WindowEvent::RedrawRequested => {
                self.surface.window.request_redraw();
                Some(DisplayEvent::RedrawRequested)
            }
            WindowEvent::CloseRequested | WindowEvent::Destroyed => Some(DisplayEvent::Quit),
            WindowEvent::KeyboardInput { event, .. } => self.process_keyevent(event),
            _ => None,
        }
    }

    pub fn process_keyevent(&mut self, event: &KeyEvent) -> Option<DisplayEvent> {
        let PhysicalKey::Code(keycode) = event.physical_key else {
            return None;
        };
        if event.repeat {
            return None;
        }
        match event.state {
            ElementState::Pressed => {
                if let KeyCode::Escape = keycode {
                    Some(DisplayEvent::Quit)
                } else {
                    self.keymap
                        .get_hotkey(keycode)
                        .map(|hotkey| DisplayEvent::Hotkey((hotkey, true)))
                }
            }
            ElementState::Released => self
                .keymap
                .get_hotkey(keycode)
                .map(|hotkey| DisplayEvent::Hotkey((hotkey, false))),
        }
    }

    pub fn toggle_frame_limiter(&mut self) {
        self.frame_limiter.limit_framerate = !self.frame_limiter.limit_framerate;
    }
}

struct Surface<const W: u32, const H: u32> {
    window: Arc<WinitWindow>,
    pixels: Pixels<'static>,
}

impl<const W: u32, const H: u32> Surface<W, H> {
    fn new(event_loop: &EventLoop<()>, scale_factor: u32) -> Result<Self> {
        event_loop.set_control_flow(ControlFlow::Poll);
        let size = LogicalSize::new((W * scale_factor) as f64, (H * scale_factor) as f64);
        let window = Arc::new(
            WindowBuilder::new()
                .with_inner_size(size)
                .with_min_inner_size(size)
                .with_resizable(false)
                .with_title("rgb")
                .build(event_loop)?,
        );

        let pixels = {
            let physical_window_size = window.inner_size();
            let surface_texture = SurfaceTexture::new(
                physical_window_size.width,
                physical_window_size.height,
                Arc::clone(&window),
            );
            Pixels::new(W, H, surface_texture)?
        };

        Ok(Self { window, pixels })
    }
}

struct FrameLimiter {
    limit_framerate: bool,
    frame_limiter: Interval,
    frame_time: Option<Duration>,
    instant: Instant,
}

impl FrameLimiter {
    fn run(&mut self, mut task: impl FnMut() -> Result<()>) -> Result<()> {
        while self.instant.elapsed() < self.frame_time.unwrap_or_default() {
            task()?;
            if self.limit_framerate {
                break;
            }
        }
        Ok(())
    }

    fn tick(&mut self) {
        if self.frame_time.is_none() {
            self.frame_time = Some(self.instant.elapsed());
        }
        if self.limit_framerate {
            self.frame_limiter.tick();
        }
        self.instant = Instant::now();
    }
}
