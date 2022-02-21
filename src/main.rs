use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat,
};

fn main() {
    let host = cpal::default_host();
    let input_device = host
        .default_input_device()
        .expect("no input device available");

    println!("Using input device: \"{}\"", input_device.name().unwrap());

    let mut supported_configs_range = input_device
        .supported_input_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .find(|supported_range| return supported_range.sample_format() == SampleFormat::I16)
        .expect("no supported config?!")
        .with_max_sample_rate();
    let config = supported_config.config();

    let file = Arc::new(Mutex::new(WavFile::new(
        config.channels,
        config.sample_rate.0,
    )));

    let file_thread = file.clone();
    let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);
    let input_stream = input_device
        .build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let mut file = file_thread.lock().unwrap();
                for &sample in data {
                    file.push_sample(sample);
                }
            },
            err_fn,
        )
        .unwrap();

    input_stream.play().expect("failed to play input stream");

    // println!("Press Ctrl+C to quit...");
    // loop {}

    std::io::stdin().read_line(&mut String::new()).unwrap();

    let file = file.lock().unwrap();
    let mut raw = vec![0u8; file.needed_size()];
    file.serialize(&mut raw)
        .expect("Failed to serialize .wav file");

    let mut output = std::fs::File::create("out.wav").expect("failed to create output file");
    output
        .write_all(&raw)
        .expect("failed to write data to file");
}

struct WavFile {
    format: WavFormat,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    samples: Vec<i16>,
}

impl WavFile {
    fn new(channels: u16, sample_rate: u32) -> Self {
        Self {
            format: WavFormat::PCM,
            channels,
            sample_rate,
            bits_per_sample: 16,
            samples: Vec::new(),
        }
    }

    fn push_sample(&mut self, sample: i16) {
        self.samples.push(sample);
    }
}

trait BinarySerialize {
    fn needed_size(&self) -> usize;
    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()>;
}

impl BinarySerialize for WavFile {
    fn needed_size(&self) -> usize {
        44 + self.samples.needed_size()
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        buffer[0..4].copy_from_slice(b"RIFF");
        let file_size = (self.needed_size() - 8) as u32;
        buffer[4..8].copy_from_slice(&file_size.to_le_bytes());
        buffer[8..12].copy_from_slice(b"WAVE");
        buffer[12..16].copy_from_slice(b"fmt ");
        // Hardcoded size, good enough here
        16u32.serialize(&mut buffer[16..20])?;
        self.format.serialize(&mut buffer[20..22])?;
        self.channels.serialize(&mut buffer[22..24])?;
        self.sample_rate.serialize(&mut buffer[24..28])?;

        let avg_bytes_per_sec =
            (self.sample_rate * self.bits_per_sample as u32 * self.channels as u32) / 8;
        avg_bytes_per_sec.serialize(&mut buffer[28..32])?;

        let block_align = (self.bits_per_sample * self.channels) / 8;
        block_align.serialize(&mut buffer[32..34])?;

        self.bits_per_sample.serialize(&mut buffer[34..36])?;
        buffer[36..40].copy_from_slice(b"data");

        let data_size = self.samples.needed_size() as u32;
        buffer[40..44].copy_from_slice(&data_size.to_le_bytes());

        self.samples.serialize(&mut buffer[44..])?;

        Ok(())
    }
}

#[repr(u16)]
#[derive(Clone, Copy)]
enum WavFormat {
    PCM = 1,
}

impl BinarySerialize for WavFormat {
    fn needed_size(&self) -> usize {
        2
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        (*self as u16).serialize(buffer)?;

        Ok(())
    }
}

impl<T: BinarySerialize> BinarySerialize for Vec<T> {
    fn needed_size(&self) -> usize {
        if self.len() == 0 {
            return 0;
        }

        self.len() * self[0].needed_size()
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        let mut off = 0;
        for val in self {
            val.serialize(&mut buffer[off..off + val.needed_size()])?;
            off += val.needed_size();
        }

        Ok(())
    }
}

impl BinarySerialize for u32 {
    fn needed_size(&self) -> usize {
        4
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        buffer[0..4].copy_from_slice(&self.to_le_bytes());

        Ok(())
    }
}

impl BinarySerialize for u16 {
    fn needed_size(&self) -> usize {
        2
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        buffer[0..2].copy_from_slice(&self.to_le_bytes());

        Ok(())
    }
}

impl BinarySerialize for i16 {
    fn needed_size(&self) -> usize {
        2
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        buffer[0..2].copy_from_slice(&self.to_le_bytes());

        Ok(())
    }
}

impl BinarySerialize for u8 {
    fn needed_size(&self) -> usize {
        1
    }

    fn serialize(&self, buffer: &mut [u8]) -> Result<(), ()> {
        if buffer.len() < self.needed_size() {
            return Err(());
        }

        buffer[0] = *self;

        Ok(())
    }
}
