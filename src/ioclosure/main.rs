// Example that processes some effect for some output in one closure function


use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf;

enum AudioIOKind {
    Input,
    Output,
}
struct CpalState {
    kind: AudioIOKind,
    device: cpal::Device,
    stream: Option<cpal::Stream>,
}

struct AudioIOState {
    input: CpalState,
    output: CpalState,
    // ringbuffer: ringbuf::HeapRb<S>,
}
impl AudioIOState {
    fn play(&mut self) -> Result<(), cpal::PlayStreamError> {
        if let Some(i) = &self.input.stream {
            i.play()?;
        };
        if let Some(o) = &self.output.stream {
            o.play()
        } else {
            Ok(())
        }
    }
    fn pause(&mut self) -> Result<(), cpal::PauseStreamError> {
        if let Some(i) = &self.input.stream {
            i.pause()?;
        };
        if let Some(o) = &self.output.stream {
            o.pause()
        } else {
            Ok(())
        }
    }
}
fn build_io_stream<S, FIO>(
    mut f: FIO,
    latency_samples: usize,
    input_channels: usize,
    output_channels: usize,
) -> AudioIOState
where
    S: Clone + Copy + cpal::Sample + Send + 'static,
    FIO: FnMut(&[S], &mut [S], usize, usize, usize) + Send + 'static,
{
    let channels_max = std::cmp::max(input_channels, output_channels);
    let rbuf = ringbuf::SharedRb::<S, Vec<_>>::new(latency_samples * 2 * channels_max);
    let (mut producer, mut consumer) = rbuf.split();

    //init cpal.
    let host = cpal::default_host();
    let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);

    //init input
    let (istream, idevice) = {
        let ifn = move |data: &[S], _: &cpal::InputCallbackInfo| {
            let pushed_samples = producer.push_slice(data);
            println!(
                "Producer: pushed_samples {:?},data.len(): {:?}",
                pushed_samples,
                data.len()
            );
            if pushed_samples < data.len() {
                eprintln!(
                    "Audio buffer overflow. {} samples were not pushed.",
                    data.len() - pushed_samples
                );
            }
        };
        let idevice = host
            .default_input_device()
            .expect("no input device available");
        let isupported_config = idevice.default_input_config().unwrap();
        println!("input config: {:?}", isupported_config);

        let iconfig = cpal::StreamConfig {
            buffer_size: cpal::BufferSize::Fixed(latency_samples as u32),
            channels: input_channels as u16,
            sample_rate: isupported_config.sample_rate(),
        };
        let st = idevice.build_input_stream(&iconfig, ifn, err_fn).unwrap();
        (st, idevice)
    };

    //init output
    let (ostream, odevice) = {
        let mut tmp_array = vec![S::from(&0u16); latency_samples * channels_max];
        let ofn = move |data: &mut [S], _: &cpal::OutputCallbackInfo| {
            //apply process function
            let consumed_samples = consumer.pop_slice(tmp_array.as_mut_slice());
            println!(
                "Consumer:  pulled_samples {:?},data.len(): {:?}",
                consumed_samples,
                data.len()
            );
            f(
                tmp_array.as_slice(),
                data,
                latency_samples,
                input_channels,
                output_channels,
            );

            let unconsumed_samples = data.len() - consumed_samples;
            if unconsumed_samples > 0 {
                eprintln!(
                    "Audio Underflow Detected, {} samples were not read.",
                    unconsumed_samples
                );
            }
        };
        let odevice = host
            .default_output_device()
            .expect("no output device available");
        let osupported_config = odevice.default_output_config().unwrap();
        println!("output config: {:?}", osupported_config);
        let myconfig = cpal::StreamConfig {
            buffer_size: cpal::BufferSize::Fixed(latency_samples as u32),
            channels: output_channels as u16,
            sample_rate: osupported_config.sample_rate(),
        };

        let ostream = odevice.build_output_stream(&myconfig, ofn, err_fn).unwrap();
        (ostream, odevice)
    };

    let mut res = AudioIOState {
        input: CpalState {
            kind: AudioIOKind::Input,
            device: idevice,
            stream: Some(istream),
        },
        output: CpalState {
            kind: AudioIOKind::Output,
            device: odevice,
            stream: Some(ostream),
        },
        // ringbuffer: rbuf,
    };
    let _ = res.pause();
    res
}

fn main() {
    let latency = 1024;
    let mut state = build_io_stream(
        |input: &[f32], output: &mut [f32], _latency, input_chs, output_chs| {
            if input_chs == output_chs {
                output.iter_mut().zip(input.iter()).for_each(|(o, i)| {
                    *o = *i * 0.5;
                })
            } else {
                for (och, ich) in output.chunks_mut(output_chs).zip(input.chunks(input_chs)) {
                    for o in och.iter_mut() {
                        *o = *ich.iter().next().unwrap_or(&0.0);
                    }
                }
            }
        },
        latency,
        2,
        2,
    );
    let _ = state.play();
    std::thread::sleep(std::time::Duration::from_millis(10000));
}
