use crate::decoding::Pos;
use piston_window::{EventLoop, PistonWindow, WindowSettings};
use plotters::prelude::*;
use plotters_piston::draw_piston_window;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

pub const LENGTH_VIEW_DATA: usize = 100;

#[derive(Debug, Clone)]
pub struct NavigationView {
    pub prn: usize,
    pub acq_mag: f32,
    pub trk_I_P: VecDeque<f32>,
    pub trk_Q_P: VecDeque<f32>,
    pub pos: Pos,
}

impl NavigationView {
    pub fn new(prn: usize) -> Self {
        Self {
            prn: prn,
            acq_mag: 0.0,
            trk_I_P: VecDeque::from([0.0]),
            trk_Q_P: VecDeque::from([0.0]),
            pos: Pos::new(),
        }
    }
}

pub fn data_view(nav_view_thread: Arc<Mutex<NavigationView>>) {
    let fps_max = 6;
    let nav_view = nav_view_thread
        .lock()
        .expect("Error in locking trk_result in data_view");
    let mut window: PistonWindow = WindowSettings::new("Realtime GNSS-SDR-RS", [450, 300])
        .samples(4)
        .build()
        .unwrap();

    window.set_max_fps(fps_max);

    let mut sat_visibility = HashMap::new();
    sat_visibility.insert(nav_view.prn, nav_view.acq_mag);
}