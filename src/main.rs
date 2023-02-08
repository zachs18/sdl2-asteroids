use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use std::time::Duration;

fn hue_to_color(hue: u32) -> Color {
    if hue <= 255 {
        Color::RGB(255, hue as u8, 0)
    } else if hue <= 255 * 2 {
        Color::RGB((255 * 2 - hue) as u8, 255, 0)
    } else if hue <= 255 * 3 {
        Color::RGB(0, 255, (hue - 255 * 2) as u8)
    } else if hue <= 255 * 4 {
        Color::RGB(0, (255 * 4 - hue) as u8, 255)
    } else if hue <= 255 * 5 {
        Color::RGB((hue - 255 * 4) as u8, 0, 255)
    } else {
        Color::RGB(255, 0, (255 * 6 - hue) as u8)
    }
}

pub fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("rust-sdl2 demo", 800, 600)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut hue = 0;
    'running: loop {
        hue = (hue + 1) % (255 * 6);
        canvas.set_draw_color(hue_to_color(hue));
        canvas.clear();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
