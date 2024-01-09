use crate::cpu::Cpu;
use crate::ppu::Ppu;
use anyhow::Result;
use pixels::{Pixels, SurfaceTexture};
use spin_sleep::LoopHelper;
use std::time::Instant;
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
    window: Window,
    input: WinitInputHelper,
    pixels: Pixels,
    limit_framerate: bool,
    frame_limiter: LoopHelper,
    frame_time: Option<f64>,
    instant: Instant,
}

impl<const W: u32, const H: u32> Display<W, H> {
    pub fn new(event_loop: EventLoop<()>) -> Self {
        event_loop.set_control_flow(ControlFlow::Poll);
        let size = LogicalSize::new((W * SCALE) as f64, (H * SCALE) as f64);
        let window = WindowBuilder::new()
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();

        let pixels = {
            let physical_window_size = window.inner_size();
            let surface_texture = SurfaceTexture::new(
                physical_window_size.width,
                physical_window_size.height,
                &window,
            );
            Pixels::new(W, H, surface_texture).unwrap()
        };

        Self {
            event_loop,
            window,
            input: WinitInputHelper::new(),
            pixels,
            limit_framerate: true,
            frame_limiter: LoopHelper::builder()
                .report_interval_s(1.0)
                .build_with_target_rate(FRAMERATE),
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
                    while self.instant.elapsed().as_secs_f64() < self.frame_time.unwrap_or_default()
                    {
                        catch_err(&mut || update(&mut cpu), elwt);
                        if self.limit_framerate {
                            break;
                        }
                    }
                    catch_err(&mut || render(cpu.ppu_mut(), &mut self.pixels), elwt);
                    if self.frame_time.is_none() {
                        self.frame_time = Some(self.instant.elapsed().as_secs_f64());
                    } else if self.limit_framerate {
                        self.frame_limiter.loop_sleep();
                        self.frame_limiter.loop_start();
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
