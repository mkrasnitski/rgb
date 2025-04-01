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

const DUTY_CYCLES: [u8; 4] = [
    0b00000001, // 12.5%
    0b00000011, // 25%
    0b00001111, // 50%
    0b11111100, // 75%
];

pub struct Apu {
    sampler: Sampler,
    channel1: channel1::Channel1,
    channel2: channel2::Channel2,
    channel3: channel3::Channel3,
    channel4: channel4::Channel4,
    aram: [u8; 0x10],
    vin_left: bool,
    left_volume: u8,
    vin_right: bool,
    right_volume: u8,
    sound_panning: u8,
    master_enable: bool,
}

impl Apu {
    pub fn new() -> Self {
        let (sample_tx, sample_rx) = channel();
        std::thread::spawn(move || spawn_audio(sample_rx));
        Self {
            sampler: Sampler::new(sample_tx),
            channel1: Default::default(),
            channel2: Default::default(),
            channel3: Default::default(),
            channel4: Default::default(),
            aram: [0; 0x10],
            vin_left: false,
            left_volume: 0,
            vin_right: false,
            right_volume: 0,
            sound_panning: 0,
            master_enable: false,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff10..=0xff14 => self.channel1.read(addr),
            0xff16..=0xff19 => self.channel2.read(addr),
            0xff1a..=0xff1e => self.channel3.read(addr),
            0xff20..=0xff23 => self.channel4.read(addr),

            0xff24 => {
                ((self.vin_left as u8) << 7)
                    | (self.left_volume << 4)
                    | ((self.vin_right as u8) << 3)
                    | self.right_volume
            }
            0xff25 => self.sound_panning,
            0xff26 => {
                ((self.master_enable as u8) << 7)
                    | 0b01110000
                    | ((self.channel4.enabled() as u8) << 3)
                    | ((self.channel3.enabled() as u8) << 2)
                    | ((self.channel2.enabled() as u8) << 1)
                    | self.channel1.enabled() as u8
            }

            0xff30..=0xff3f => self.aram[addr as usize - 0xff30],
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
            0xff1a..=0xff1e => self.channel3.write(addr, val),
            0xff20..=0xff23 => self.channel4.write(addr, val),

            0xff24 => {
                self.vin_left = val.bit(7);
                self.left_volume = (val >> 4) & 0b111;
                self.vin_right = val.bit(3);
                self.right_volume = val & 0b111;
            }
            0xff25 => self.sound_panning = val,
            0xff26 => {
                let bit = val.bit(7);
                self.master_enable = bit;
                if !bit {
                    // Powering off APU resets all registers to 0
                    self.channel1 = Default::default();
                    self.channel2 = Default::default();
                    self.channel3 = Default::default();
                    self.channel4 = Default::default();
                    self.vin_left = false;
                    self.left_volume = 0;
                    self.vin_right = false;
                    self.right_volume = 0;
                    self.sound_panning = 0;
                }
            }

            0xff30..=0xff3f => self.aram[addr as usize - 0xff30] = val,
            _ => unreachable!(),
        }
    }

    pub fn tick(&mut self) {
        self.channel1.tick();
        self.channel2.tick();
        self.channel3.tick();
        self.channel3.tick();
        self.channel4.tick();

        let left_volume = (self.left_volume as f32 + 1.0) / 8.0;
        let right_volume = (self.right_volume as f32 + 1.0) / 8.0;
        let total_volume = (left_volume + right_volume) / 2.0;

        let sample = (self.channel1.sample()
            + self.channel2.sample()
            + self.channel3.sample(&self.aram)
            + self.channel4.sample())
            / 4.0;
        self.sampler.push_sample(sample * total_volume * 0.5);
    }

    pub fn tick_frame_sequencer(&mut self) {
        self.channel1.tick_frame_sequencer();
        self.channel2.tick_frame_sequencer();
        self.channel3.tick_frame_sequencer();
        self.channel4.tick_frame_sequencer();
    }

    pub fn toggle_frame_limiter(&mut self) {
        self.sampler.limit_framerate = !self.sampler.limit_framerate;
    }
}

fn spawn_audio(sample_rx: Receiver<f32>) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    let config = StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(2) {
                if let Ok(sample) = sample_rx.recv() {
                    for channel in frame.iter_mut() {
                        *channel = sample;
                    }
                }
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
    sample_tx: Sender<f32>,
    sample_buffer: Vec<f32>,
    instant: Instant,
    limit_framerate: bool,
}

impl Sampler {
    fn new(sample_tx: Sender<f32>) -> Self {
        Self {
            sample_tx,
            sample_buffer: Vec::with_capacity(8192),
            instant: Instant::now(),
            limit_framerate: true,
        }
    }

    fn push_sample(&mut self, sample: f32) {
        if self.sample_buffer.len() < 8192 {
            self.sample_buffer.push(sample);
        }
        if self.sample_buffer.len() == 8192
            && (self.limit_framerate
                || self.instant.elapsed() >= Duration::from_secs_f64(1.0 / 128.0))
        {
            // 8192 samples @ 1048576Hz = 375 samples @ 48000Hz
            //
            // Interpolate 22 or 21 samples at a time.
            //   8192 = 317*22 + 58*21
            //   317 + 58 = 375
            let (h1, h2) = self.sample_buffer.split_at(317 * 22);
            let samples: [f32; 375] = h1
                .chunks_exact(22)
                .chain(h2.chunks_exact(21))
                .map(|slice| slice.iter().sum::<f32>() / slice.len() as f32)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            for sample in samples {
                self.sample_tx.send(sample).unwrap();
            }
            self.sample_buffer.clear();
            self.instant = Instant::now();
        }
    }
}
