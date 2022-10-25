use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use rand::Rng;

use std::sync::{Arc, Mutex};
#[derive(Default)]
struct AudioParameter {
    freq: f32,
    amp: f32,
}
#[derive(Default)]
struct Oscillo {
    samples: Vec<f32>,
}

fn main() {
    let buf_size: usize = 512;
    let chs: usize = 2;

    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");
    let supported_config = device.default_output_config().unwrap();
    let myconfig = cpal::StreamConfig {
        buffer_size: cpal::BufferSize::Fixed(buf_size as u32),
        channels: chs as u16,
        sample_rate: supported_config.sample_rate(),
    };

    let params = Arc::new(Mutex::new(AudioParameter {
        freq: 440.0,
        amp: 1.0,
    }));
    let scope = Arc::new(Mutex::new(Oscillo {
        samples: vec![0.0; buf_size * chs],
    }));
    let scope_th = Arc::clone(&scope);
    println!(
        "Device: {}, Using config: {:?}",
        device.name().expect("unknown"),
        myconfig
    );
    let params_th = Arc::clone(&params);

    let mut counter = 0.0;
    let cls = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut amp = 0.0;
        if let Ok(l) = params_th.try_lock() {
            amp = l.amp;
        }
        for buf in data.chunks_mut(myconfig.channels as usize) {
            for (chcount, sample) in buf.iter_mut().enumerate() {
                let s = counter % 1.0 * amp;
                *sample = Sample::from(&s);
                if chcount == 1 {
                    counter = counter + 0.01;
                }
            }
        }
        if let Ok(mut s) = scope_th.try_lock() {
            s.samples.copy_from_slice(data);
        }
        // println!("{:?}", data);
    };
    let stream = device.build_output_stream(&myconfig, cls, err_fn).unwrap();
    stream.play().unwrap();

    loop {
        let mut sleepflag = true;
        let _ = match Arc::clone(&scope).try_lock() {
            Ok(l) => {
                if let Ok(mut p_lock) = params.try_lock() {
                    let mut rng = rand::thread_rng();
                    let i: u32 = rng.gen();
                    p_lock.amp = i as f32 / (1u32 << 31u32) as f32;
                }
                println!("{:?}", l.samples);
                sleepflag = true;
            }
            Err(e) => {
                eprintln!("{}", e);
                sleepflag = false;
            }
        };
        if sleepflag {
            std::thread::sleep(std::time::Duration::from_millis(2000));
        }
    }
}