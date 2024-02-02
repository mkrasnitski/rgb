use crate::cpu::Cpu;
use crate::ppu::Ppu;
use anyhow::Result;
use pixels::{Pixels, SurfaceTexture};
use spin_sleep_util::Interval;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::KeyCode,
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

const SCALE: u32 = 3;
const FRAMERATE: f64 = 4194304.0 / 70224.0;

pub struct Display<const W: u32, const H: u32> {
    event_loop: EventLoop<()>,
    window: Arc<Window>,
    input: WinitInputHelper,
    pixels: Pixels<'static>,
    limit_framerate: bool,
    frame_limiter: Interval,
    frame_time: Option<Duration>,
    instant: Instant,
}

impl<const W: u32, const H: u32> Display<W, H> {
    pub fn new(event_loop: EventLoop<()>) -> Self {
        event_loop.set_control_flow(ControlFlow::Poll);
        let size = LogicalSize::new((W * SCALE) as f64, (H * SCALE) as f64);
        let window = Arc::new(
            WindowBuilder::new()
                .with_inner_size(size)
                .with_min_inner_size(size)
                .with_resizable(false)
                .build(&event_loop)
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
            event_loop,
            window,
            input: WinitInputHelper::new(),
            pixels,
            limit_framerate: true,
            frame_limiter: spin_sleep_util::interval(Duration::from_secs_f64(1.0 / FRAMERATE)),
            frame_time: None,
            instant: Instant::now(),
        }
    }

    pub fn run(
        mut self,
        mut cpu: Cpu,
        mut update: impl FnMut(&mut Cpu) -> Result<()>,
        mut render: impl FnMut(&mut Ppu, &mut Pixels) -> Result<()>,
    ) -> Result<()> {
        Ok(self.event_loop.run(|event, elwt| {
            match event {
                Event::AboutToWait => {
                    self.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    while self.instant.elapsed() < self.frame_time.unwrap_or_default() {
                        catch_err(&mut || update(&mut cpu), elwt);
                        if self.limit_framerate {
                            break;
                        }
                    }
                    catch_err(&mut || render(cpu.ppu_mut(), &mut self.pixels), elwt);
                    if self.frame_time.is_none() {
                        self.frame_time = Some(self.instant.elapsed());
                    }
                    if self.limit_framerate {
                        self.frame_limiter.tick();
                    }
                    self.instant = Instant::now();
                }
                _ => {}
            }

            if self.input.update(&event) {
                if self.input.key_pressed(KeyCode::Escape) || self.input.close_requested() {
                    elwt.exit();
                }

                if self.input.key_pressed(KeyCode::Space) {
                    self.limit_framerate = !self.limit_framerate;
                }
            }
        })?)
    }
}

fn catch_err<F, T>(f: &mut F, elwt: &EventLoopWindowTarget<()>)
where
    F: FnMut() -> Result<T>,
{
    if let Err(e) = f() {
        println!("{e:?}");
        elwt.exit();
    }
}
