use crate::tracking::TrackingResult;
use std::error::Error;
use std::sync::{Arc, Mutex};

pub struct Pos {
    x: f32,
    y: f32,
    z: f32,
    t: f32,
}

pub fn nav_decoding(tracking_result: Arc<Mutex<TrackingResult>>) -> Result<Pos, Box<dyn Error>> {
    todo!();
}
