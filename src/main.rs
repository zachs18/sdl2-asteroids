use arrayvec::ArrayVec;
use as_point::AsPoint;
use either::Either;
use glam::{DMat2, DVec2, UVec2};
use itertools::Itertools;
use rand::Rng;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use std::sync::Arc;
use std::time::Duration;

mod as_point;

const FPS: u32 = 60;

#[derive(Default, Clone, Copy)]
struct Body {
    position: DVec2,
    velocity: DVec2,
    /// in radians, clockwise from north
    rotation: f64,
    has_drag: bool,
    accelerating: bool,
    turning_left: bool,
    turning_right: bool,
}

type Verts = Either<&'static [DVec2], Arc<[DVec2]>>;
type Triangles = Either<&'static [[DVec2; 3]], Arc<[[DVec2; 3]]>>;

struct Polygon {
    /// Offsets from origin, cyclic
    verts: Verts,
}

enum Bounding {
    /// This bounding box consists of N triangles, each with one vertex at the origin,
    /// and two others consecutive elements of the cyclic list `verts`.
    CyclicTriangles { verts: Verts },
    /// This bounding box consists of N triangles, each explicitly listed.
    Triangles { triangles: Triangles },
}

enum WrappingBehavior {
    Yes,
    No,
    /// Wrapping should change to `Yes` once this entity is entirely on-screen,
    /// but should behave as `No` until then.
    OnceOnScreen,
}

struct Entity {
    body: Body,
    /// Should drawing and moving this entity wrap around the screen.
    wrap: WrappingBehavior,
    sprite_verts: Option<Polygon>,
    bounding: Option<Bounding>,
    kind: EntityKind,
}

#[derive(Debug, Clone, Copy)]
enum EntityKind {
    Asteroid {
        /// Decremented by 1 each time the asteroid splits, until it is gone.
        size: usize,
    },
    Bullet {
        /// Time to live, in frames
        ttl: u64,
    },
    Debris {
        /// Time to live, in frames
        ttl: u64,
    },
    Player {
        fire: Option<Keycode>,
        accelerate: Option<Keycode>,
        turn_left: Option<Keycode>,
        turn_right: Option<Keycode>,
    },
}

const BULLET_VERTS: Verts = Either::Left(&[
    DVec2 { x: 1.0, y: 3.0 },
    DVec2 { x: 1.0, y: -3.0 },
    DVec2 { x: -1.0, y: -3.0 },
    DVec2 { x: -1.0, y: 3.0 },
]);

const BULLET_BOUNDS: Bounding = Bounding::Triangles {
    triangles: Either::Left(&[
        [
            DVec2 { x: 1.0, y: 3.0 },
            DVec2 { x: 1.0, y: -3.0 },
            DVec2 { x: -1.0, y: -3.0 },
        ],
        [
            DVec2 { x: 1.0, y: -3.0 },
            DVec2 { x: -1.0, y: -3.0 },
            DVec2 { x: -1.0, y: 3.0 },
        ],
    ]),
};

const SHIP_VERTS: Verts = Either::Left(&[
    DVec2 { x: 0.0, y: -20.0 },
    DVec2 { x: 10.0, y: 10.0 },
    DVec2 { x: 0.0, y: 0.0 },
    DVec2 { x: -10.0, y: 10.0 },
]);

fn asteroid_verts(vert_count: usize, min_distance: f64, max_distance: f64) -> Verts {
    assert!(vert_count >= 3);
    let mut rng = rand::thread_rng();
    let theta_increment = std::f64::consts::TAU / (vert_count as f64);
    Either::Right(
        (0..vert_count)
            .map(|idx| {
                rotation_matrix(theta_increment * idx as f64)
                    * DVec2 {
                        x: 0.0,
                        y: rng.gen_range(min_distance..=max_distance),
                    }
            })
            .collect(),
    )
}

fn new_debris(body: Body) -> Entity {
    let verts = asteroid_verts(9, 2.0, 5.0);
    Entity {
        body,
        wrap: WrappingBehavior::Yes,
        sprite_verts: Some(Polygon {
            verts: verts.clone(),
        }),
        bounding: Some(Bounding::CyclicTriangles { verts }),
        kind: EntityKind::Debris { ttl: 30 },
    }
}

fn new_asteroid(size: usize, body: Body) -> Entity {
    let verts = match size {
        0 => panic!("Invalid asteroid size"),
        1 => asteroid_verts(6, 20.0, 28.0),
        2 => asteroid_verts(8, 30.0, 40.0),
        3 => asteroid_verts(14, 39.0, 50.0),
        _ => unreachable!(),
    };
    Entity {
        body,
        wrap: WrappingBehavior::Yes,
        sprite_verts: Some(Polygon {
            verts: verts.clone(),
        }),
        bounding: Some(Bounding::CyclicTriangles { verts }),
        kind: EntityKind::Asteroid { size },
    }
}

enum StepResult {
    None,
    RemoveEntity,
}

impl Entity {
    fn handle_event(&mut self, event: &Event) -> Vec<Entity> {
        let mut new_entities = vec![];
        match self.kind {
            EntityKind::Player {
                fire,
                accelerate,
                turn_left,
                turn_right,
            } => match event {
                &Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    if Some(keycode) == accelerate {
                        self.body.accelerating = true;
                    } else if Some(keycode) == turn_left {
                        self.body.turning_left = true;
                    } else if Some(keycode) == turn_right {
                        self.body.turning_right = true;
                    } else if Some(keycode) == fire {
                        let fire_direction =
                            rotation_matrix(self.body.rotation) * DVec2 { x: 0.0, y: -1.0 };
                        new_entities.push(Entity {
                            body: Body {
                                position: self.body.position + fire_direction * 20.0,
                                velocity: fire_direction * 4.0 + self.body.velocity,
                                rotation: self.body.rotation,
                                has_drag: false,
                                accelerating: false,
                                turning_left: false,
                                turning_right: false,
                            },
                            wrap: WrappingBehavior::Yes,
                            sprite_verts: Some(Polygon {
                                verts: BULLET_VERTS,
                            }),
                            bounding: Some(BULLET_BOUNDS),
                            kind: EntityKind::Bullet { ttl: 120 },
                        })
                    }
                }
                &Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    if Some(keycode) == accelerate {
                        self.body.accelerating = false;
                    } else if Some(keycode) == turn_left {
                        self.body.turning_left = false;
                    } else if Some(keycode) == turn_right {
                        self.body.turning_right = false;
                    }
                }
                _ => {}
            },
            EntityKind::Asteroid { .. } => {}
            EntityKind::Bullet { .. } => {}
            EntityKind::Debris { .. } => {} // _ => todo!(),
        }
        new_entities
    }

    fn step(&mut self, bounds: DVec2) -> StepResult {
        if self.body.accelerating {
            let rota = rotation_matrix(self.body.rotation);
            self.body.velocity += rota * DVec2 { x: 0.0, y: -0.1 };
        }
        match (self.body.turning_left, self.body.turning_right) {
            (false, true) => {
                // Rotate at 1/3 rotations per second
                self.body.rotation = (self.body.rotation
                    - std::f64::consts::TAU / (FPS * 3) as f64)
                    .rem_euclid(std::f64::consts::TAU);
            }
            (true, false) => {
                // Rotate at 1/3 rotations per second
                self.body.rotation = (self.body.rotation + std::f64::consts::TAU / (FPS * 3) as f64)
                    .rem_euclid(std::f64::consts::TAU)
            }
            _ => {}
        }

        if self.body.has_drag {
            self.body.velocity *= 0.99;
        }

        self.body.position += self.body.velocity;
        match self.wrap {
            WrappingBehavior::No => {}
            WrappingBehavior::Yes => {
                // self.position = self.position.rem_euclid(bounds);
                self.body.position.x = self.body.position.x.rem_euclid(bounds.x);
                self.body.position.y = self.body.position.y.rem_euclid(bounds.y);
            }
            WrappingBehavior::OnceOnScreen => {
                let (min_self_x, max_self_x, min_self_y, max_self_y) = self
                    .bounding_triangles()
                    .flatten()
                    .chain([self.body.position])
                    .fold(
                        (
                            f64::INFINITY,
                            f64::NEG_INFINITY,
                            f64::INFINITY,
                            f64::NEG_INFINITY,
                        ),
                        |(min_self_x, max_self_x, min_self_y, max_self_y), DVec2 { x, y }| {
                            (
                                x.min(min_self_x),
                                x.max(max_self_x),
                                y.min(min_self_y),
                                y.max(max_self_y),
                            )
                        },
                    );
                if min_self_x >= 0.0
                    && min_self_y >= 0.0
                    && max_self_x <= bounds.x
                    && max_self_y <= bounds.y
                {
                    self.wrap = WrappingBehavior::Yes;
                }
            }
        }
        match &mut self.kind {
            EntityKind::Asteroid { .. } => {}
            EntityKind::Bullet { ttl } | EntityKind::Debris { ttl } => match ttl.checked_sub(1) {
                Some(new_ttl) => *ttl = new_ttl,
                None => return StepResult::RemoveEntity,
            },
            EntityKind::Player { .. } => {}
        }
        StepResult::None
    }

    fn bounding_triangles(&self) -> impl Iterator<Item = [DVec2; 3]> + Clone + '_ {
        // type Ret = Either<_, std::iter::Empty<T>>;
        let Some(bounding) = &self.bounding else { return Either::Right(Either::Right(std::iter::empty())) };
        let rota = rotation_matrix(self.body.rotation);
        let origin = self.body.position;
        match bounding {
            Bounding::CyclicTriangles { verts } => {
                // // https://github.com/rust-itertools/itertools/issues/685
                // let triangles = verts
                //     .iter()
                //     .copied()
                //     .circular_tuple_windows()
                //     .map(move |(p1, p2)| [origin, origin + rota * p1, origin + rota * p2]);
                let triangles = verts
                    .iter()
                    .cycle()
                    .tuple_windows()
                    .take(verts.len())
                    .map(move |(&p1, &p2)| [origin, origin + rota * p1, origin + rota * p2]);

                Either::Left(triangles)
            }
            Bounding::Triangles { triangles } => {
                let triangles = triangles.iter().map(move |&[p1, p2, p3]| {
                    [origin + rota * p1, origin + rota * p2, origin + rota * p3]
                });

                Either::Right(Either::Left(triangles))
            }
        }
    }

    /// Returns true if self and other may collide, i.e. if they do anything when they overlap.
    fn collides_with(&self, other: &Self) -> bool {
        match (self.kind, other.kind) {
            (EntityKind::Debris { .. }, _) | (_, EntityKind::Debris { .. }) => false,
            (EntityKind::Asteroid { .. }, EntityKind::Asteroid { .. }) => false,
            (EntityKind::Bullet { .. }, EntityKind::Bullet { .. }) => false,

            (EntityKind::Bullet { .. }, EntityKind::Asteroid { .. }) => true,
            (EntityKind::Asteroid { .. }, EntityKind::Bullet { .. }) => true,
            (EntityKind::Asteroid { .. }, EntityKind::Player { .. }) => true,
            (EntityKind::Bullet { .. }, EntityKind::Player { .. }) => true,
            (EntityKind::Player { .. }, EntityKind::Asteroid { .. }) => true,
            (EntityKind::Player { .. }, EntityKind::Bullet { .. }) => true,
            (EntityKind::Player { .. }, EntityKind::Player { .. }) => true,
        }
    }

    fn collision(&self, other: &Self) -> bool {
        for self_triangle in self.bounding_triangles() {
            // Simple fast-negative check
            let (min_self_x, max_self_x, min_self_y, max_self_y) = self_triangle.iter().fold(
                (
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(min_self_x, max_self_x, min_self_y, max_self_y), &DVec2 { x, y }| {
                    (
                        x.min(min_self_x),
                        x.max(max_self_x),
                        y.min(min_self_y),
                        y.max(max_self_y),
                    )
                },
            );
            for other_triangle in other.bounding_triangles() {
                // Simple fast-negative check
                let (min_other_x, max_other_x, min_other_y, max_other_y) =
                    other_triangle.iter().fold(
                        (
                            f64::INFINITY,
                            f64::NEG_INFINITY,
                            f64::INFINITY,
                            f64::NEG_INFINITY,
                        ),
                        |(min_other_x, max_other_x, min_other_y, max_other_y), &DVec2 { x, y }| {
                            (
                                x.min(min_other_x),
                                x.max(max_other_x),
                                y.min(min_other_y),
                                y.max(max_other_y),
                            )
                        },
                    );

                if max_self_x < min_other_x
                    || min_self_x > max_other_x
                    || max_self_y < min_other_y
                    || min_self_y > max_other_y
                {
                    continue;
                }

                let all_points = [
                    self_triangle[0] - other_triangle[0],
                    self_triangle[0] - other_triangle[1],
                    self_triangle[0] - other_triangle[2],
                    self_triangle[1] - other_triangle[0],
                    self_triangle[1] - other_triangle[1],
                    self_triangle[1] - other_triangle[2],
                    self_triangle[2] - other_triangle[0],
                    self_triangle[2] - other_triangle[1],
                    self_triangle[2] - other_triangle[2],
                ];

                // TODO: GJK algorithm? (see Reducible video)
                eprintln!("TODO: actual collision");
                return true;
            }
        }
        false
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

pub fn rotation_matrix(theta: f64) -> DMat2 {
    DMat2 {
        x_axis: DVec2 {
            x: theta.cos(),
            y: -theta.sin(),
        },
        y_axis: DVec2 {
            x: theta.sin(),
            y: theta.cos(),
        },
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

    canvas.set_draw_color(Color::BLACK);
    canvas.clear();
    canvas.present();

    let (handle_tx, handle_rx) = tokio::sync::oneshot::channel();
    let runtime_thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to initialize tokio runtime");
        let runtime = &runtime;
        runtime.block_on(async move {
            let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
            handle_tx.send((stop_tx, runtime.handle().clone())).unwrap();
            let mut interval = tokio::time::interval(Duration::new(1, 0) / FPS);
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
    let mut frame_interval = tokio::time::interval(Duration::new(1, 0) / 60);
    let mut entities = vec![
        Entity {
            sprite_verts: Some(Polygon { verts: SHIP_VERTS }),
            bounding: Some(Bounding::CyclicTriangles { verts: SHIP_VERTS }),
            wrap: WrappingBehavior::Yes,
            body: Body {
                position: DVec2 { x: 300.0, y: 300.0 },
                has_drag: true,
                ..Default::default()
            },
            kind: EntityKind::Player {
                accelerate: Some(Keycode::Up),
                turn_right: Some(Keycode::Right),
                turn_left: Some(Keycode::Left),
                fire: Some(Keycode::Space),
            },
        },
        Entity {
            sprite_verts: Some(Polygon { verts: SHIP_VERTS }),
            bounding: Some(Bounding::CyclicTriangles { verts: SHIP_VERTS }),
            wrap: WrappingBehavior::Yes,
            body: Body {
                position: DVec2 { x: 500.0, y: 300.0 },
                has_drag: true,
                ..Default::default()
            },
            kind: EntityKind::Player {
                accelerate: Some(Keycode::W),
                turn_right: Some(Keycode::D),
                turn_left: Some(Keycode::A),
                fire: Some(Keycode::LCtrl),
            },
        },
        new_asteroid(
            3,
            Body {
                position: DVec2::default(),
                velocity: DVec2 { x: -1.0, y: 2.2 },
                rotation: 0.0,
                has_drag: false,
                accelerating: false,
                turning_left: false,
                turning_right: false,
            },
        ),
        new_asteroid(
            2,
            Body {
                position: DVec2::default(),
                velocity: DVec2 { x: 1.0, y: 1.2 },
                rotation: 0.0,
                has_drag: false,
                accelerating: false,
                turning_left: false,
                turning_right: false,
            },
        ),
        new_asteroid(
            1,
            Body {
                position: DVec2::default(),
                velocity: DVec2 { x: 2.0, y: -1.6 },
                rotation: 0.0,
                has_drag: false,
                accelerating: false,
                turning_left: false,
                turning_right: false,
            },
        ),
    ];

    'running: loop {
        let draw_color = Color::WHITE;
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();
        for event in event_pump.poll_iter() {
            let new_entities = entities
                .iter_mut()
                .flat_map(|entity| entity.handle_event(&event))
                .collect::<Vec<_>>();
            entities.extend(new_entities);
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

        let bounds: UVec2 = canvas.output_size().unwrap().into();
        let bounds: DVec2 = bounds.as_dvec2();

        entities.retain_mut(|entity| match entity.step(bounds) {
            StepResult::RemoveEntity => false,
            StepResult::None => true,
        });

        macro_rules! split_asteroid {
            (asteroid = $asteroid:expr, bullet = $bullet:expr) => {
                let asteroid = $asteroid;
                let bullet = $bullet;
                let EntityKind::Asteroid { size } = asteroid.kind else { unreachable!() };
                dbg!(size);
                if size > 1 {
                    dbg!(size);
                    let split_direction = bullet.body.rotation + std::f64::consts::FRAC_PI_2;
                    let mut left_asteroid = new_asteroid(size - 1, asteroid.body);
                    let mut right_asteroid = new_asteroid(size - 1, asteroid.body);
                    let rota = rotation_matrix(split_direction);
                    let left = rota * DVec2 { x: 0.0, y: 1.0 };
                    let right = -left;
                    left_asteroid.body.velocity += left;
                    left_asteroid.body.position += left;
                    right_asteroid.body.velocity += right;
                    right_asteroid.body.position += right;
                    entities.extend([left_asteroid, right_asteroid]);
                }
                for _ in 0..size * 4 - 2 {
                    let debris_direction =
                        rand::thread_rng().gen_range(0.0..=std::f64::consts::TAU);
                    let rota = rotation_matrix(debris_direction);
                    let velocity_offset = rota * DVec2 { x: 0.0, y: 4.0 };
                    let mut body = asteroid.body;
                    body.velocity += velocity_offset;
                    entities.push(new_debris(body));
                }
            };
            (bullet = $bullet:expr, asteroid = $asteroid:expr) => {
                let bullet = $bullet;
                let asteroid = $asteroid;
                split_asteroid!(asteroid = asteroid, bullet = bullet);
            };
        }

        // TODO: collisions
        let mut i = 0;
        while i < entities.len() {
            let mut j = 0;
            while i < entities.len() && j < i {
                if entities[i].collides_with(&entities[j]) && entities[i].collision(&entities[j]) {
                    match (entities[i].kind, entities[j].kind) {
                        (EntityKind::Debris { .. }, _) | (_, EntityKind::Debris { .. }) => {}
                        (EntityKind::Asteroid { .. }, EntityKind::Asteroid { .. }) => {}
                        (EntityKind::Bullet { .. }, EntityKind::Bullet { .. }) => {}
                        (EntityKind::Bullet { .. }, EntityKind::Asteroid { .. }) => {
                            let bullet = entities.swap_remove(i.max(j));
                            let asteroid = entities.swap_remove(i.min(j));
                            split_asteroid!(asteroid = asteroid, bullet = bullet);
                        }
                        (EntityKind::Asteroid { .. }, EntityKind::Bullet { .. }) => {
                            let asteroid = entities.swap_remove(i.max(j));
                            let bullet = entities.swap_remove(i.min(j));
                            split_asteroid!(asteroid = asteroid, bullet = bullet);
                        }
                        (
                            EntityKind::Asteroid { size },
                            EntityKind::Player {
                                fire,
                                accelerate,
                                turn_left,
                                turn_right,
                            },
                        ) => eprintln!("TODO: Player at {:?} dies", entities[j].body.position),
                        (
                            EntityKind::Bullet { ttl },
                            EntityKind::Player {
                                fire,
                                accelerate,
                                turn_left,
                                turn_right,
                            },
                        ) => eprintln!("TODO: Player at {:?} dies", entities[j].body.position),
                        (
                            EntityKind::Player {
                                fire,
                                accelerate,
                                turn_left,
                                turn_right,
                            },
                            EntityKind::Asteroid { size },
                        ) => eprintln!("TODO: Player at {:?} dies", entities[i].body.position),
                        (
                            EntityKind::Player {
                                fire,
                                accelerate,
                                turn_left,
                                turn_right,
                            },
                            EntityKind::Bullet { ttl },
                        ) => eprintln!("TODO: Player at {:?} dies", entities[i].body.position),
                        (EntityKind::Player { .. }, EntityKind::Player { .. }) => {
                            eprintln!("TODO: Players collided")
                        }
                    }
                }

                j += 1;
            }
            i += 1;
        }

        // entities.sort_unstable_by_key(|entity| float_ord::FloatOrd(entity.body.position.y));
        for entity in &entities {
            let pos = entity.body.position;
            let rota = rotation_matrix(entity.body.rotation);

            // canvas.set_draw_color(hue_to_color((hue + entity.color_offset) % (255 * 6)));
            canvas.set_draw_color(draw_color);

            if let Some(verts) = &entity.sprite_verts {
                for (p1, p2) in verts.verts.iter().copied().circular_tuple_windows() {
                    let p1 = rota * p1 + pos;
                    let p2 = rota * p2 + pos;
                    if !matches!(entity.wrap, WrappingBehavior::Yes) {
                        canvas.draw_line(p1.as_point(), p2.as_point()).ok();
                    } else {
                        let minx = p1.x.min(p2.x);
                        let maxx = p1.x.max(p2.x);
                        let miny = p1.y.min(p2.y);
                        let maxy = p1.y.max(p2.y);
                        let mut dxs: ArrayVec<i32, 3> = ArrayVec::from_iter([0]);
                        let mut dys: ArrayVec<i32, 3> = ArrayVec::from_iter([0]);
                        if minx < 0.0 {
                            // If the line is at all above the top edge, copy it down to the bottom edge
                            dxs.push(1);
                        }
                        if maxx > bounds.x {
                            // If the line is at all below the bottom edge, copy it up to the top edge
                            dxs.push(-1);
                        }
                        if miny < 0.0 {
                            // If the line is at left of above the left edge, copy it right to the right edge
                            dys.push(1);
                        }
                        if maxy > bounds.y {
                            // If the line is at all right of the right edge, copy it left to the left edge
                            dys.push(-1);
                        }
                        for dy in dys {
                            for dx in dxs.clone() {
                                let mult = DVec2 {
                                    x: dx as f64,
                                    y: dy as f64,
                                };
                                let offset = bounds * mult;
                                let p1 = p1 + offset;
                                let p2 = p2 + offset;
                                canvas.draw_line(p1.as_point(), p2.as_point()).ok();
                            }
                        }
                    }
                }
            }

            // canvas
            //     .fill_rect(Rect::new(x as i32 - 40, y as i32 - 40, 80, 80))
            //     .ok();
        }

        canvas.present();
        handle.block_on(frame_interval.tick());
    }
    stop_tx.send(true).ok();
    runtime_thread.join().unwrap();
}
