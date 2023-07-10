use crate::tracking::TrackingStatistics;
use std::collections::HashMap;
use std::error::Error;
pub struct Pos {
    x: f32,
    y: f32,
    z: f32,
    t: f32,
}

pub fn nav_decoding(
    tracking_result: HashMap<i16, TrackingStatistics>,
) -> Result<Pos, Box<dyn Error>> {
    todo!();
}
