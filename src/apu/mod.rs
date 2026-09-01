use crate::utils::BitExtract;

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::Result;
use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

mod channel1;
mod channel2;
mod channel3;
mod channel4;
mod utils;

const SAMPLERATE: u32 = 48000;

const DUTY_CYCLES: [u8; 4] = [
    0b00000001, // 12.5%
    0b00000011, // 25%
    0b00001111, // 50%
    0b11111100, // 75%
];

pub struct Apu {
    sampler: Option<Sampler>,
    channel1: channel1::Channel1,
    channel2: channel2::Channel2,
    channel3: channel3::Channel3,
    channel4: channel4::Channel4,
    panning: Panning,
    vin_left: bool,
    left_volume: u8,
    vin_right: bool,
    right_volume: u8,
    master_enable: bool,
}

#[derive(Default)]
struct Panning {
    channel1: (bool, bool),
    channel2: (bool, bool),
    channel3: (bool, bool),
    channel4: (bool, bool),
}

impl Panning {
    fn new(val: u8) -> Self {
        Self {
            channel1: (val.bit(4), val.bit(0)),
            channel2: (val.bit(5), val.bit(1)),
            channel3: (val.bit(6), val.bit(2)),
            channel4: (val.bit(7), val.bit(3)),
        }
    }

    fn as_u8(&self) -> u8 {
        ((self.channel4.0 as u8) << 7)
            | ((self.channel3.0 as u8) << 6)
            | ((self.channel2.0 as u8) << 5)
            | ((self.channel1.0 as u8) << 4)
            | ((self.channel4.1 as u8) << 3)
            | ((self.channel3.1 as u8) << 2)
            | ((self.channel2.1 as u8) << 1)
            | self.channel1.1 as u8
    }
}

impl Apu {
    pub fn new(volume: f32, disable_audio: bool, limit_framerate: bool) -> Self {
        let sampler = if disable_audio {
            None
        } else {
            let (sample_tx, sample_rx) = channel();
            std::thread::spawn(move || spawn_audio(sample_rx, volume));
            Some(Sampler::new(sample_tx, limit_framerate))
        };
        Self {
            sampler,
            channel1: Default::default(),
            channel2: Default::default(),
            channel3: Default::default(),
            channel4: Default::default(),
            panning: Default::default(),
            vin_left: false,
            left_volume: 0,
            vin_right: false,
            right_volume: 0,
            master_enable: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff10..=0xff14 => self.channel1.read(addr),
            0xff16..=0xff19 => self.channel2.read(addr),
            0xff1a..=0xff1e | 0xff30..=0xff3f => self.channel3.read(addr),
            0xff20..=0xff23 => self.channel4.read(addr),

            0xff24 => {
                ((self.vin_left as u8) << 7)
                    | (self.left_volume << 4)
                    | ((self.vin_right as u8) << 3)
                    | self.right_volume
            }
            0xff25 => self.panning.as_u8(),
            0xff26 => {
                ((self.master_enable as u8) << 7)
                    | 0b01110000
                    | ((self.channel4.enabled() as u8) << 3)
                    | ((self.channel3.enabled() as u8) << 2)
                    | ((self.channel2.enabled() as u8) << 1)
                    | self.channel1.enabled() as u8
            }

            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if !(self.master_enable || addr == 0xff26 || (0xff30..=0xff3f).contains(&addr)) {
            return;
        }

        match addr {
            0xff10..=0xff14 => self.channel1.write(addr, val),
            0xff16..=0xff19 => self.channel2.write(addr, val),
            0xff1a..=0xff1e | 0xff30..=0xff3f => self.channel3.write(addr, val),
            0xff20..=0xff23 => self.channel4.write(addr, val),

            0xff24 => {
                self.vin_left = val.bit(7);
                self.left_volume = (val >> 4) & 0b111;
                self.vin_right = val.bit(3);
                self.right_volume = val & 0b111;
            }
            0xff25 => self.panning = Panning::new(val),
            0xff26 => {
                let bit = val.bit(7);
                if !bit {
                    // Powering off APU resets all registers to 0
                    self.channel1.power_off();
                    self.channel2.power_off();
                    self.channel3.power_off();
                    self.channel4.power_off();
                    self.panning = Panning::default();
                    self.vin_left = false;
                    self.left_volume = 0;
                    self.vin_right = false;
                    self.right_volume = 0;
                } else if !self.master_enable {
                    self.channel1.power_on();
                    self.channel2.power_on();
                    self.channel3.power_on();
                    self.channel4.power_on();
                }
                self.master_enable = bit;
            }
            _ => unreachable!(),
        }
    }

    pub fn tick(&mut self) {
        self.channel1.tick();
        self.channel2.tick();
        self.channel3.tick(); // Tick channel 3 twice per mcycle
        self.channel3.tick();
        self.channel4.tick();

        let left_vol = (self.left_volume as f32 + 1.0) / 8.0;
        let right_vol = (self.right_volume as f32 + 1.0) / 8.0;
        let (left_sample, right_sample) = self.sample();

        if let Some(sampler) = &mut self.sampler {
            sampler.push_sample((left_sample * left_vol, right_sample * right_vol));
        }
    }

    fn sample(&self) -> (f32, f32) {
        macro_rules! pan {
            ($chan:ident) => {{
                let sample = self.$chan.sample();
                let (pan_left, pan_right) = self.panning.$chan;
                (
                    if pan_left { sample } else { 0.0 },
                    if pan_right { sample } else { 0.0 },
                )
            }};
        }

        let (ch1_left, ch1_right) = pan!(channel1);
        let (ch2_left, ch2_right) = pan!(channel2);
        let (ch3_left, ch3_right) = pan!(channel3);
        let (ch4_left, ch4_right) = pan!(channel4);

        let left = (ch1_left + ch2_left + ch3_left + ch4_left) / 4.0;
        let right = (ch1_right + ch2_right + ch3_right + ch4_right) / 4.0;
        (left, right)
    }

    pub fn tick_frame_sequencer(&mut self) {
        self.channel1.tick_frame_sequencer();
        self.channel2.tick_frame_sequencer();
        self.channel3.tick_frame_sequencer();
        self.channel4.tick_frame_sequencer();
    }

    pub fn toggle_frame_limiter(&mut self) {
        if let Some(sampler) = &mut self.sampler {
            sampler.limit_framerate = !sampler.limit_framerate;
        }
    }
}

fn spawn_audio(sample_rx: Receiver<(f32, f32)>, volume: f32) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    let config = StreamConfig {
        channels: 2,
        sample_rate: SAMPLERATE,
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = device.build_output_stream(
        config,
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(2) {
                let (left, right) = sample_rx.try_recv().unwrap_or_default();
                frame[0] = left * volume / 100.0;
                frame[1] = right * volume / 100.0;
            }
        },
        |err| eprintln!("{err}"),
        None,
    )?;
    stream.play()?;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

struct Sampler {
    sample_tx: Sender<(f32, f32)>,
    sample_buffer: Vec<(f32, f32)>,
    instant: Instant,
    limit_framerate: bool,
}

impl Sampler {
    fn new(sample_tx: Sender<(f32, f32)>, limit_framerate: bool) -> Self {
        Self {
            sample_tx,
            sample_buffer: Vec::with_capacity(8192),
            instant: Instant::now(),
            limit_framerate,
        }
    }

    fn push_sample(&mut self, sample: (f32, f32)) {
        if self.sample_buffer.len() < 8192 {
            self.sample_buffer.push(sample);
        }
        if self.sample_buffer.len() == 8192
            && (self.limit_framerate
                || self.instant.elapsed() >= Duration::from_secs_f64(1.0 / 128.0))
        {
            // 8192 samples @ 1048576Hz = 375 samples @ 48000Hz
            for i in 0..375 {
                let sample = self.sample_buffer[i * 8192 / 375];
                let _ = self.sample_tx.send(sample);
            }
            self.sample_buffer.clear();
            self.instant = Instant::now();
        }
    }
}
