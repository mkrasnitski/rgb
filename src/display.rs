use crate::cpu::Cpu;
use anyhow::Result;
use pixels::{Pixels, SurfaceTexture};
use spin_sleep_util::Interval;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window as WinitWindow, WindowBuilder},
};

const SCALE: u32 = 3;
const FRAMERATE: f64 = 4194304.0 / 70224.0;

pub struct Display<const W: u32, const H: u32> {
    event_loop: EventLoop<()>,
    window: Window<W, H>,
}

impl<const W: u32, const H: u32> Display<W, H> {
    pub fn new(event_loop: EventLoop<()>) -> Self {
        let display = Window::new(&event_loop);
        Self {
            event_loop,
            window: display,
        }
    }

    pub fn run(mut self, mut cpu: Cpu) -> Result<()> {
        Ok(self.event_loop.run(|event, elwt| match event {
            Event::AboutToWait => {
                self.window.window.request_redraw();
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::RedrawRequested => {
                    if let Err(e) = self.window.draw_frame(&mut cpu) {
                        println!("{e:?}");
                        elwt.exit();
                    }
                }
                WindowEvent::CloseRequested | WindowEvent::Destroyed => elwt.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if self.window.handle_keyevent(event) {
                        elwt.exit()
                    }
                }
                _ => {}
            },
            _ => {}
        })?)
    }
}

struct Window<const W: u32, const H: u32> {
    window: Arc<WinitWindow>,
    pixels: Pixels<'static>,
    frame_limiter: FrameLimiter,
}

impl<const W: u32, const H: u32> Window<W, H> {
    fn new(event_loop: &EventLoop<()>) -> Self {
        event_loop.set_control_flow(ControlFlow::Poll);
        let size = LogicalSize::new((W * SCALE) as f64, (H * SCALE) as f64);
        let window = Arc::new(
            WindowBuilder::new()
                .with_inner_size(size)
                .with_min_inner_size(size)
                .with_resizable(false)
                .build(event_loop)
                .unwrap(),
        );

        let pixels = {
            let physical_window_size = window.inner_size();
            let surface_texture = SurfaceTexture::new(
                physical_window_size.width,
                physical_window_size.height,
                Arc::clone(&window),
            );
            Pixels::new(W, H, surface_texture).unwrap()
        };

        Self {
            window,
            pixels,
            frame_limiter: FrameLimiter {
                limit_framerate: true,
                frame_limiter: spin_sleep_util::interval(Duration::from_secs_f64(1.0 / FRAMERATE)),
                frame_time: None,
                instant: Instant::now(),
            },
        }
    }

    fn handle_keyevent(&mut self, event: KeyEvent) -> bool {
        let PhysicalKey::Code(keycode) = event.physical_key else {
            return false;
        };
        if let ElementState::Pressed = event.state {
            match keycode {
                KeyCode::Escape => return true,
                KeyCode::Space => {
                    self.frame_limiter.limit_framerate = !self.frame_limiter.limit_framerate
                }
                _ => {}
            }
        }
        false
    }

    fn draw_frame(&mut self, cpu: &mut Cpu) -> Result<()> {
        self.frame_limiter.run(|| cpu.run_frame())?;
        cpu.ppu_mut().render(&mut self.pixels)?;
        self.frame_limiter.tick();
        Ok(())
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
