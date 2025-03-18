use crate::utils::BitExtract;

use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::Result;
use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

mod channel1;
mod channel2;
mod channel3;
mod channel4;

const DUTY_CYCLES: [u8; 4] = [
    0b00000001, // 12.5%
    0b00000011, // 25%
    0b00001111, // 50%
    0b11111100, // 75%
];

pub struct Apu {
    channel1: channel1::Channel1,
    channel2: channel2::Channel2,
    channel3: channel3::Channel3,
    channel4: channel4::Channel4,
    aram: [u8; 0x10],
    master_volume_vin_panning: u8,
    sound_panning: u8,
    master_enable: bool,

    sample_tx: Sender<[f32; 375]>,
    sample_buffer: Vec<f32>,
}

impl Apu {
    pub fn new() -> Self {
        let (sample_tx, sample_rx) = channel();
        std::thread::spawn(move || spawn_audio(sample_rx));
        Self {
            channel1: Default::default(),
            channel2: Default::default(),
            channel3: Default::default(),
            channel4: Default::default(),
            aram: [0; 0x10],
            master_volume_vin_panning: 0,
            sound_panning: 0,
            master_enable: false,

            sample_tx,
            sample_buffer: Vec::with_capacity(8192),
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xff10..=0xff14 => self.channel1.read(addr),
            0xff16..=0xff19 => self.channel2.read(addr),
            0xff1a..=0xff1e => self.channel3.read(addr),
            0xff20..=0xff23 => self.channel4.read(addr),

            0xff24 => self.master_volume_vin_panning,
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
        if addr != 0xff26 && !self.master_enable {
            return;
        }

        match addr {
            0xff10..=0xff14 => self.channel1.write(addr, val),
            0xff16..=0xff19 => self.channel2.write(addr, val),
            0xff1a..=0xff1e => self.channel3.write(addr, val),
            0xff20..=0xff23 => self.channel4.write(addr, val),

            0xff24 => self.master_volume_vin_panning = val,
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
                    self.master_volume_vin_panning = 0;
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

        self.sample_buffer
            .push(self.channel1.sample() * 0.2 + self.channel2.sample() * 0.2);
        if self.sample_buffer.len() == 8192 {
            self.send_samples()
        }
    }

    pub fn tick_frame_sequencer(&mut self) {
        self.channel1.tick_frame_sequencer();
        self.channel2.tick_frame_sequencer();
    }

    fn send_samples(&mut self) {
        // 8192 samples @ 1048576Hz = 375 samples @ 48000Hz
        //
        // Interpolate 22 or 21 samples at a time.
        //   8192 = 317*22 + 58*21
        //   317 + 58 = 375
        let (h1, h2) = self.sample_buffer.split_at(317 * 22);
        let samples = h1
            .chunks_exact(22)
            .chain(h2.chunks_exact(21))
            .map(|slice| slice.iter().sum::<f32>() / slice.len() as f32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        self.sample_buffer.clear();
        self.sample_tx.send(samples).unwrap()
    }
}

struct SampleIterator<const N: usize> {
    sample_rx: Receiver<[f32; N]>,
    buffer: Option<[f32; N]>,
    index: usize,
}

impl<const N: usize> SampleIterator<N> {
    fn new(sample_rx: Receiver<[f32; N]>) -> Self {
        Self {
            sample_rx,
            buffer: None,
            index: 0,
        }
    }
}

impl<const N: usize> Iterator for SampleIterator<N> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.is_none() || self.index >= N {
            self.buffer = Some(self.sample_rx.recv().ok()?);
            self.index = 0;
        }
        let val = self.buffer.map(|buf| buf[self.index]);
        self.index += 1;
        val
    }
}

fn spawn_audio(sample_rx: Receiver<[f32; 375]>) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
    let config = StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut sample_iter = SampleIterator::new(sample_rx);
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(2) {
                if let Some(sample) = sample_iter.next() {
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
