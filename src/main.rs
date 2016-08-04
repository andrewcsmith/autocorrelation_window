extern crate piston;
extern crate piston_window;
extern crate graphics;
extern crate gfx_graphics;
extern crate portaudio;
extern crate vox_box;
extern crate bounded_spsc_queue as spsc;
extern crate rustfft as fft;
extern crate num;

use std::thread;
use std::sync::{Arc, TryLockError, LockResult, PoisonError};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ops::Deref;

use piston::input::*;
use piston::window::{Window, AdvancedWindow, WindowSettings};
use piston_window::{PistonWindow};

use spsc::{Producer, Consumer};

use vox_box::periodic::Autocorrelate;
use vox_box::waves::Normalize;

const INTERLEAVED: bool = true;
const LATENCY: portaudio::Time = 0.0;
const CHANNELS: i32 = 1;
const FRAMES_PER_BUFFER: u32 = 64;
const SAMPLE_RATE: f64 = 44100.0;

const RING_BUFFER_SIZE: u32 = FRAMES_PER_BUFFER * 64;
const AC_BUFFER_SIZE: u32 = FRAMES_PER_BUFFER * 16;
const AUTO_COEFFS: usize = 64;
const FFT_SIZE: usize = 256;
const HALF_FFT_SIZE: usize = 256 / 2;

fn main() {
    let mut window: PistonWindow = WindowSettings::new("Hello Piston!", [640, 480])
        .exit_on_esc(true).fullscreen(true).samples(1).build().unwrap();
    // window.draw_2d(|c, g| piston_window::clear(graphics::color::WHITE, g));

    println!("Press x to stop.");

    let (producer, consumer) = spsc::make::<f64>(AUTO_COEFFS as usize);
    let (fft_producer, fft_consumer) = spsc::make::<f64>(FFT_SIZE);
    {
        thread::spawn(move || run(producer, fft_producer).unwrap());
    }

    while let Some(e) = window.next() {
        match e {
            Event::Input(i) => {
                match i {
                    Input::Press(Button::Keyboard(keyboard::Key::X)) => {
                        window.set_should_close(true);
                    },
                    _ => {  }
                }
            },
            Event::Render(RenderArgs { ext_dt, width, height, .. } ) => {
                let mut shared_buf = [0f64; AUTO_COEFFS];
                let mut fft_buf = [0f64; FFT_SIZE];

                // copies AUTO_COEFFS values to local array
                let mut index: usize = 0;
                while let Some(val) = consumer.try_pop() {
                    shared_buf[index & (AUTO_COEFFS-1)] = val;
                    index += 1;
                }

                // Get the fft spectrum values
                index = 0;
                while let Some(val) = fft_consumer.try_pop() {
                    fft_buf[index & (FFT_SIZE-1)] = val;
                    index += 1;
                }

                window.draw_2d(&e, |c, g| {
                    piston_window::clear(graphics::color::WHITE, g);
                    let mut length = shared_buf.len() as f64 - 1.;
                    let mut dx = (1. / length) * width as f64;
                    for (i, v) in shared_buf.windows(2).enumerate() {
                        let x1 = i as f64 * dx;
                        let x2 = x1 + dx;
                        let y1 = height as f64 - (height as f64 * (v[0] + 1.)) / 2.;
                        let y2 = height as f64 - (height as f64 * (v[1] + 1.)) / 2.;
                        let line = piston_window::Line::new_round([0., 0., 3., 1.], 1.);
                        let dims = [x1, y1, x2, y2];
                        line.draw(dims, &c.draw_state, c.transform, g);
                    }
                    length = fft_buf.len() as f64 - 1.;
                    dx = (1. / length) * width as f64;
                    for (i, v) in fft_buf.iter().enumerate() {
                        let x1 = (i as f64 * dx) + dx / 2.;
                        let x2 = x1;
                        let y1 = height as f64 - (height as f64 * (v + 1.));
                        let y2 = height as f64;
                        let line = piston_window::Line::new_round([0., 3., 0., 0.5], dx / 4.);
                        let dims = [x1, y1, x2, y2];
                        line.draw(dims, &c.draw_state, c.transform, g);
                    }
                });
            },
            _ => {  }
        }
    }
}

struct Mel {
    pow: f64,
    mel: f64
}

fn run(producer: Producer<f64>, fft_producer: Producer<f64>) -> Result<(), portaudio::Error> {
    let pa = try!(portaudio::PortAudio::new());
    println!("Found PortAudio version {}", pa.version());
    for device in pa.devices().unwrap() {
        let (idx, info) = device.unwrap();
        println!("{}: {}", idx.0, info.name);
    }

    let mut auto_buffer = VecDeque::<f32>::with_capacity(AC_BUFFER_SIZE as usize);
    let mut fft = fft::FFT::new(FFT_SIZE, false);
    let mut spectrum = vec![num::Complex { re: 0., im: 0.}; FFT_SIZE];
    let mut cepstrum = spectrum.clone();
    let mut signal = spectrum.clone();
    let mut settings: portaudio::DuplexStreamSettings<f32, f32> = try!(pa.default_duplex_stream_settings(CHANNELS, CHANNELS, SAMPLE_RATE, FRAMES_PER_BUFFER));
    settings.flags = portaudio::stream_flags::CLIP_OFF;

    let callback = move |portaudio::DuplexStreamCallbackArgs { in_buffer, out_buffer, frames, .. }| {
        // Make room in the autocorrelation buffer
        while auto_buffer.capacity() < (auto_buffer.len() + frames) { auto_buffer.pop_front(); }
        for i in 0..frames { auto_buffer.push_back(in_buffer[i]); }
        for i in 0..frames { signal[i].re = in_buffer[i]; }
        let mut auto_coeffs: [f32; AUTO_COEFFS] = [0.; AUTO_COEFFS];
        auto_buffer.autocorrelate_mut(&mut auto_coeffs[..]);
        auto_coeffs.normalize();
        fft.process(&signal, &mut spectrum);

        for val in auto_coeffs.iter() { 
            producer.try_push(*val as f64);
        }

        for val in spectrum[0..((FFT_SIZE as f64 / 2.).floor() as usize)].iter() {
            fft_producer.try_push(val.norm_sqr() as f64);
        }

        for i in 0..(frames * CHANNELS as usize) {
            out_buffer[i] = in_buffer[i];
        }
        portaudio::Continue
    };

    let mut stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    try!(stream.start());

    loop { }

    Ok(())
}
