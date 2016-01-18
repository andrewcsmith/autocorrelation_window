extern crate piston;
extern crate piston_window;
extern crate graphics;
extern crate gfx_graphics;
extern crate portaudio;
extern crate vox_box;

use std::thread;
use std::sync::Arc;

use piston::input::*;
use piston::window::{Window as Win, AdvancedWindow, WindowSettings};
use piston_window::{PistonWindow as Window};

fn main() {
    let mut window: Window = WindowSettings::new("Hello Piston!", [640, 480])
        .exit_on_esc(true).fullscreen(true).build().unwrap();

    println!("Press x to stop.");

    let mut samp = Arc::new(0f64);
    thread::spawn(|| run().unwrap());

    for w in window {
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
                        let rectangle = piston_window::Rectangle::new([0.5; 4]);
                        piston_window::clear(graphics::color::WHITE, g);
                        rectangle.draw([width as f64 / 2 as f64, height as f64 / 2 as f64, 10., 10.], &c.draw_state, c.transform, g); 
                    });
                },
                _ => {  }
            }
        }
    }
}

const INTERLEAVED: bool = true;
const LATENCY: portaudio::Time = 0.0;
const CHANNELS: i32 = 2;
const FRAMES_PER_BUFFER: u32 = 64;
const SAMPLE_RATE: f64 = 44100.0;

fn run() -> Result<(), portaudio::Error> {
    let pa = try!(portaudio::PortAudio::new());
    println!("Found PortAudio version {}", pa.version());
    for device in pa.devices().unwrap() {
        let (idx, info) = device.unwrap();
        println!("{}: {}", idx.0, info.name);
    }

    let mut settings: portaudio::InputStreamSettings<f32> = try!(pa.default_input_stream_settings(CHANNELS, SAMPLE_RATE, FRAMES_PER_BUFFER));
    settings.flags = portaudio::stream_flags::CLIP_OFF;

    let callback = move |portaudio::InputStreamCallbackArgs { buffer, frames, .. }| {
        // println!("Got {} frames", frames);
        portaudio::Continue
    };

    let mut stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    stream.start();

    Ok(())
}
