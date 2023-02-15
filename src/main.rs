use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
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

    let mut window = video_subsystem
        .window("rust-sdl2 demo", 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    window.set_minimum_size(100, 100).unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(hue_to_color(0));
    canvas.clear();
    canvas.present();

    let (handle_tx, handle_rx) = tokio::sync::oneshot::channel();
    let runtime_thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to initialize tokio runtime");
        let runtime = &runtime;
        runtime.block_on(async move {
            let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
            handle_tx.send((stop_tx, runtime.handle().clone())).unwrap();
            let mut interval = tokio::time::interval(Duration::new(1, 0) / 60);
            loop {
                if *stop_rx.borrow_and_update() {
                    break;
                }
                interval.tick().await;
            }
        })
    });

    // Not really used yet, except for keeping the frame interval mostly constant.
    let Ok((stop_tx, handle)) = handle_rx.blocking_recv() else {
        drop(canvas);
        drop(sdl_context);
        panic!("Failed to initialize communication with tokio runtime");
    };
    let _enterguard = handle.enter();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut hue = 0;
    let mut frame_interval = tokio::time::interval(Duration::new(1, 0) / 60);
    let mut position = (40.0, 40.0);
    let mut velocity = (0.0, 0.0);
    let mut acceleration = (0.0, 0.0);

    #[derive(Default)]
    struct DirectionButtons {
        up: bool,
        down: bool,
        left: bool,
        right: bool,
    }

    let mut direction_buttons = DirectionButtons::default();

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
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat,
                    ..
                } => match keycode {
                    Keycode::Up if !repeat => direction_buttons.up = true,
                    Keycode::Down if !repeat => direction_buttons.down = true,
                    Keycode::Left if !repeat => direction_buttons.left = true,
                    Keycode::Right if !repeat => direction_buttons.right = true,
                    _ => {}
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Up => direction_buttons.up = false,
                    Keycode::Down => direction_buttons.down = false,
                    Keycode::Left => direction_buttons.left = false,
                    Keycode::Right => direction_buttons.right = false,
                    _ => {}
                },
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        match (direction_buttons.up, direction_buttons.down) {
            (false, true) => {
                if velocity.1 < 0.0 {
                    velocity.1 *= 0.9;
                }
                acceleration.1 = 0.1;
            }
            (true, false) => {
                if velocity.1 > 0.0 {
                    velocity.1 *= 0.9;
                }
                acceleration.1 = -0.1;
            }
            _ => {
                velocity.1 *= 0.9;
                acceleration.1 = 0.0;
            }
        }

        match (direction_buttons.left, direction_buttons.right) {
            (false, true) => {
                if velocity.0 < 0.0 {
                    velocity.0 *= 0.9;
                }
                acceleration.0 = 0.1;
            }
            (true, false) => {
                if velocity.0 > 0.0 {
                    velocity.0 *= 0.9;
                }
                acceleration.0 = -0.1;
            }
            _ => {
                velocity.0 *= 0.9;
                acceleration.0 = 0.0;
            }
        }

        position.0 += velocity.0;
        position.1 += velocity.1;

        velocity.0 += acceleration.0;
        velocity.1 += acceleration.1;

        let (w, h) = canvas.output_size().unwrap();
        let w = w.max(80);
        let h = h.max(80);
        if position.0 < 40.0 || position.0 > (w - 40) as f64 {
            position.0 = f64::clamp(position.0, 40.0, (w - 40) as f64);
            velocity.0 = 0.0;
            acceleration.0 = 0.0;
        }
        if position.1 < 40.0 || position.1 > (h - 40) as f64 {
            position.1 = f64::clamp(position.1, 40.0, (h - 40) as f64);
            velocity.1 = 0.0;
            acceleration.1 = 0.0;
        }

        canvas.set_draw_color(hue_to_color((hue + 255 * 3) % (255 * 6)));

        canvas
            .fill_rect(Rect::new(
                position.0 as i32 - 40,
                position.1 as i32 - 40,
                80,
                80,
            ))
            .ok();

        canvas.present();
        handle.block_on(frame_interval.tick());
    }
    stop_tx.send(true).ok();
    runtime_thread.join().unwrap();
}
