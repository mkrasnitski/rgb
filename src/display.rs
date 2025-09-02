use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::cpu::Cpu;
use crate::hotkeys::{Hotkey, KeyMap};

use anyhow::Result;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use spin_sleep_util::Interval;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

const FRAMERATE: f64 = 4194304.0 / 70224.0;

pub enum DisplayEvent {
    Hotkey((Hotkey, bool)),
    RedrawRequested,
    Quit,
}

pub struct Display {
    surface: Option<Surface<160, 144>>,
    keymap: KeyMap,
    scale_factor: u32,
    limit_framerate: bool,
    frame_limiter: Interval,
    instant: Instant,
}

impl Display {
    pub fn new(keymap: KeyMap, scale_factor: u32) -> Self {
        Self {
            surface: None,
            keymap,
            scale_factor,
            limit_framerate: true,
            frame_limiter: spin_sleep_util::interval(Duration::from_secs_f64(1.0 / FRAMERATE)),
            instant: Instant::now(),
        }
    }

    pub fn reinit_surface(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        self.surface = Some(Surface::new(event_loop, self.scale_factor)?);
        Ok(())
    }

    pub fn quit(&mut self, event_loop: &ActiveEventLoop) {
        self.surface.take();
        event_loop.exit();
    }

    pub fn draw_frame(&mut self, cpu: &mut Cpu) -> Result<()> {
        if let Some(surface) = &mut self.surface {
            if self.limit_framerate {
                cpu.run_frame()?;
                self.frame_limiter.tick();
            } else {
                while self.instant.elapsed() < Duration::from_secs_f64(1.0 / 480.0) {
                    cpu.run_frame()?;
                }
            }
            cpu.ppu_mut().render(&mut surface.pixels)?;
            self.instant = Instant::now();
        }
        Ok(())
    }

    pub fn process_event(&mut self, event: &WindowEvent) -> Option<DisplayEvent> {
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(surface) = &self.surface {
                    surface.window.request_redraw();
                }
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
        self.limit_framerate = !self.limit_framerate;
    }
}

struct Surface<const W: u32, const H: u32> {
    window: Arc<Window>,
    pixels: Pixels<'static>,
}

impl<const W: u32, const H: u32> Surface<W, H> {
    fn new(event_loop: &ActiveEventLoop, scale_factor: u32) -> Result<Self> {
        event_loop.set_control_flow(ControlFlow::Poll);
        let size = LogicalSize::new((W * scale_factor) as f64, (H * scale_factor) as f64);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_inner_size(size)
                    .with_min_inner_size(size)
                    .with_resizable(false)
                    .with_title("rgb"),
            )?,
        );

        let pixels = {
            let physical_window_size = window.inner_size();
            let surface_texture = SurfaceTexture::new(
                physical_window_size.width,
                physical_window_size.height,
                Arc::clone(&window),
            );
            PixelsBuilder::new(W, H, surface_texture)
                .enable_vsync(false)
                .build()?
        };

        Ok(Self { window, pixels })
    }
}
