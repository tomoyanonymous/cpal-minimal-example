use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use rand::Rng;
use ringbuf;

use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct AudioParameter {
    freq: f32,
    amp: f32,
}

struct Oscillo {
    samples: ringbuf::HeapRb<f32>,
}

fn make_processor(
    bufsize: usize,
    channels: usize,
) -> (
    impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send,
    Arc<Mutex<AudioParameter>>,
    ringbuf::HeapConsumer<f32>,
) {
    let params = Arc::new(Mutex::new(AudioParameter {
        freq: 440.0,
        amp: 1.0,
    }));
    let scope = Oscillo {
        samples: ringbuf::HeapRb::new(bufsize * channels * 2),
    };
    let params_res = Arc::clone(&params);
    let (mut producer, consumer) = scope.samples.split();
    let mut counter = 0.0;
    //  main code called in audio thread.
    let cls = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut amp = 0.0;
        if let Ok(l) = params.try_lock() {
            amp = l.amp;
        }
        for buf in data.chunks_mut(channels as usize) {
            let phase = counter % 1.0;
            let phasor_twopi = phase * 2.0 * std::f32::consts::PI;
            let wave = phasor_twopi.sin() * amp;
            let res = Sample::from(&wave);

            counter = counter + 0.01;
            if counter > 1.0 {
                counter = counter % 1.0
            }
            buf.iter_mut().for_each(|sample| {
                *sample = res;
            });
        }
        producer.push_slice(data);
        // println!("{:?}", data);
    };
    (cls, params_res, consumer)
}

fn main() {
    let buf_size: usize = 128;
    let chs: usize = 2;
    let (processor, params, mut scope_consumer) = make_processor(buf_size, chs);

    //init cpal.
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
    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    let stream = device
        .build_output_stream(&myconfig, processor, err_fn)
        .unwrap();
    stream.play().unwrap();
    let waiting_time =
        std::time::Duration::from_secs(buf_size as u64 / myconfig.sample_rate.0 as u64);
    let mut local_scope_buf = vec![0f32; buf_size as usize * myconfig.channels as usize];
    let amp_change_period = 1000000;
    let mut period_count = 0;
    loop {
        if period_count > amp_change_period {
            //randomly update amplitude by overwriting the variable "amp" from main thread asynchronously.
            if let Ok(mut p_lock) = params.try_lock() {
                let mut rng = rand::thread_rng();
                let i: u32 = rng.gen_range(0..1024);
                p_lock.amp = i as f32 / 1024 as f32;
                println!("count: {}, {}", period_count, p_lock.amp);
            }
            period_count = 0;
        }
        // dump the samples in audio buffer that are updated in audio thread

        let _numsamples = scope_consumer.pop_slice(local_scope_buf.as_mut_slice());
        // println!("{:?}", local_scope_buf);

        std::thread::sleep(waiting_time);

        period_count += 1;
    }
}
