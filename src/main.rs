use glam::{DVec2, DVec3};
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

#[derive(Default)]
struct Body {
    position: DVec3,
    velocity: DVec3,
    acceleration: DVec3,
}

#[derive(Default)]
struct Entity {
    body: Body,
    color_offset: u32,
    north: Option<Keycode>,
    accelerating_north: bool,
    east: Option<Keycode>,
    accelerating_east: bool,
    south: Option<Keycode>,
    accelerating_south: bool,
    west: Option<Keycode>,
    accelerating_west: bool,
    jump: Option<Keycode>,
    jumping: bool,
}

impl Entity {
    fn handle_event(&mut self, event: &Event) {
        match event {
            &Event::KeyDown {
                keycode: Some(keycode),
                repeat: false,
                ..
            } => {
                if Some(keycode) == self.north {
                    self.accelerating_north = true;
                } else if Some(keycode) == self.east {
                    self.accelerating_east = true;
                } else if Some(keycode) == self.south {
                    self.accelerating_south = true;
                } else if Some(keycode) == self.west {
                    self.accelerating_west = true;
                } else if Some(keycode) == self.jump {
                    self.jumping = true;
                }
            }
            &Event::KeyUp {
                keycode: Some(keycode),
                ..
            } => {
                if Some(keycode) == self.north {
                    self.accelerating_north = false;
                } else if Some(keycode) == self.east {
                    self.accelerating_east = false;
                } else if Some(keycode) == self.south {
                    self.accelerating_south = false;
                } else if Some(keycode) == self.west {
                    self.accelerating_west = false;
                } else if Some(keycode) == self.jump {
                    self.jumping = false;
                }
            }
            _ => {}
        }
    }
    fn step(&mut self) {
        match (self.accelerating_north, self.accelerating_south) {
            (false, true) => {
                if self.body.velocity.y < 0.0 {
                    self.body.velocity.y *= 0.9;
                }
                self.body.acceleration.y = 0.1;
            }
            (true, false) => {
                if self.body.velocity.y > 0.0 {
                    self.body.velocity.y *= 0.9;
                }
                self.body.acceleration.y = -0.1;
            }
            _ => {
                self.body.velocity.y *= 0.9;
                self.body.acceleration.y = 0.0;
            }
        }
        match (self.accelerating_west, self.accelerating_east) {
            (false, true) => {
                if self.body.velocity.x < 0.0 {
                    self.body.velocity.x *= 0.9;
                }
                self.body.acceleration.x = 0.1;
            }
            (true, false) => {
                if self.body.velocity.x > 0.0 {
                    self.body.velocity.x *= 0.9;
                }
                self.body.acceleration.x = -0.1;
            }
            _ => {
                self.body.velocity.x *= 0.9;
                self.body.acceleration.x = 0.0;
            }
        }
        if self.jumping {
            if self.body.velocity.z < 0.1 && self.body.position.z < 0.1 {
                self.body.velocity.z = 8.0;
                self.body.acceleration.z = -0.3333;
            }
        }

        self.body.position += self.body.velocity;
        self.body.velocity += self.body.acceleration;
        if self.body.position.z < 0.0 {
            self.body.position.z = 0.0;
            self.body.velocity.z = 0.0;
            self.body.acceleration.z = 0.0;
        }
    }

    fn project(&self) -> DVec2 {
        DVec2 {
            x: self.body.position.x,
            y: self.body.position.y - self.body.position.z,
        }
    }
}

pub fn shade(c: Color, by: f64) -> Color {
    Color {
        r: (c.r as f64 * by) as u8,
        g: (c.g as f64 * by) as u8,
        b: (c.b as f64 * by) as u8,
        a: c.a,
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
    let mut entities = vec![
        Entity {
            color_offset: 510,
            north: Some(Keycode::Up),
            east: Some(Keycode::Right),
            south: Some(Keycode::Down),
            west: Some(Keycode::Left),
            jump: Some(Keycode::Space),
            body: Body {
                position: DVec3 {
                    x: 80.0,
                    y: 40.0,
                    z: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        },
        Entity {
            color_offset: 1020,
            north: Some(Keycode::W),
            east: Some(Keycode::D),
            south: Some(Keycode::S),
            west: Some(Keycode::A),
            jump: Some(Keycode::LCtrl),
            body: Body {
                position: DVec3 {
                    x: 240.0,
                    y: 40.0,
                    z: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        },
    ];

    'running: loop {
        hue = (hue + 1) % (255 * 6);
        canvas.set_draw_color(hue_to_color(hue));
        canvas.clear();
        for event in event_pump.poll_iter() {
            entities
                .iter_mut()
                .for_each(|entity| entity.handle_event(&event));
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

        for entity in &mut entities {
            entity.step();
        }
        entities.sort_unstable_by_key(|entity| float_ord::FloatOrd(entity.body.position.y));
        for entity in &entities {
            let shadow_x = entity.body.position.x;
            let shadow_y = entity.body.position.y;

            canvas.set_draw_color(shade(hue_to_color(hue), 0.5));

            canvas
                .fill_rect(Rect::new(
                    shadow_x as i32 - 20,
                    shadow_y as i32 + 30,
                    40,
                    20,
                ))
                .ok();
        }
        for entity in &entities {
            let DVec2 { x, y } = entity.project();

            canvas.set_draw_color(hue_to_color((hue + entity.color_offset) % (255 * 6)));

            canvas
                .fill_rect(Rect::new(x as i32 - 40, y as i32 - 40, 80, 80))
                .ok();
        }

        canvas.present();
        handle.block_on(frame_interval.tick());
    }
    stop_tx.send(true).ok();
    runtime_thread.join().unwrap();
}
