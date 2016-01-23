extern crate piston;
extern crate piston_window;
extern crate graphics;
extern crate gfx_graphics;
extern crate portaudio;
extern crate vox_box;

use std::thread;
use std::sync::{Arc, Mutex, TryLockError, LockResult, PoisonError};
use std::cell::{Cell, RefCell};
use std::ops::Deref;

use piston::input::*;
use piston::window::{Window as Win, AdvancedWindow, WindowSettings};
use piston_window::{PistonWindow as Window};

const INTERLEAVED: bool = true;
const LATENCY: portaudio::Time = 0.0;
const CHANNELS: i32 = 2;
const FRAMES_PER_BUFFER: u32 = 64;
const SAMPLE_RATE: f64 = 44100.0;

fn main() {
    let mut window: Window = WindowSettings::new("Hello Piston!", [640, 480])
        .exit_on_esc(true).fullscreen(true).samples(1).build().unwrap();

    println!("Press x to stop.");

    let mut samp = Arc::new(Mutex::new(vec![0.; FRAMES_PER_BUFFER as usize]));
    {
        let samp = samp.clone();
        thread::spawn(move || run(samp).unwrap());
    }

    for w in window {
        w.draw_2d(|c, g| piston_window::clear(graphics::color::WHITE, g));
        while let Some(e) = w.events.borrow_mut().next() {
            match e {
                Event::Input(i) => {
                    match i {
                        Input::Press(Button::Keyboard(keyboard::Key::X)) => {
                            w.window.borrow_mut().set_should_close(true);
                        },
                        _ => {  }
                    }
                },
                Event::Render(RenderArgs { ext_dt, width, height, .. } ) => {
                    w.draw_2d(|c, g| {
                        piston_window::clear(graphics::color::WHITE, g);
                        let guard = samp.lock().unwrap();
                        let shared_buf = guard.deref();
                        let length = shared_buf.len() as f64 - 1.;
                        let dx = (1. / length) * width as f64;
                        for (i, v) in shared_buf.windows(2).enumerate() {
                            let x1 = i as f64 * dx;
                            let x2 = x1 + dx;
                            let y1 = (height as f64 * (v[0] + 1.)) / 2.;
                            let y2 = (height as f64 * (v[1] + 1.)) / 2.;
                            let line = piston_window::Line::new_round([0., 0., 0., 1.], 2.);
                            let dims = [x1, y1, x2, y2];
                            // println!("line: {:?}", dims);
                            line.draw(dims, &c.draw_state, c.transform, g);
                        }
                        // let rectangle = piston_window::Rectangle::new([0.0; 4]);
                        // rectangle.draw([500., 500., 100., 100.], &c.draw_state, c.transform, g);
                    });
                },
                _ => {  }
            }
        }
    }
}

fn run(val: Arc<Mutex<Vec<f64>>>) -> Result<(), portaudio::Error> {
    let pa = try!(portaudio::PortAudio::new());
    println!("Found PortAudio version {}", pa.version());
    for device in pa.devices().unwrap() {
        let (idx, info) = device.unwrap();
        println!("{}: {}", idx.0, info.name);
    }

    let mut settings: portaudio::InputStreamSettings<f32> = try!(pa.default_input_stream_settings(CHANNELS, SAMPLE_RATE, FRAMES_PER_BUFFER));
    settings.flags = portaudio::stream_flags::CLIP_OFF;

    let callback = move |portaudio::InputStreamCallbackArgs { buffer, frames, .. }| {
        match val.lock() {
            Ok(mut shared_buf) => { 
                for i in 0..shared_buf.len() {
                    shared_buf[i] = buffer[i] as f64;
                }
            },
            Err(err) => { 
                println!("Mutex poisoned! {}", err);
            }
        }
        portaudio::Continue
    };

    let mut stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    stream.start();

    loop { }

    Ok(())
}
