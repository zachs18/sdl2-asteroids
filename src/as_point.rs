use sdl2::rect::Point;

pub trait AsPoint {
    fn as_point(&self) -> Point;
}

impl AsPoint for glam::DVec2 {
    fn as_point(&self) -> Point {
        Point::from((self.x as i32, self.y as i32))
    }
}

impl AsPoint for glam::Vec2 {
    fn as_point(&self) -> Point {
        Point::from((self.x as i32, self.y as i32))
    }
}
