use crate::comm_func::max_float_vec;
use crate::decoding::Pos;
use crossbeam_channel::Receiver;
use piston_window::*;
use plotters::prelude::*;
use plotters_bitmap::bitmap_pixel::RGBPixel;
use plotters_bitmap::BitMapBackend;
use std::borrow::BorrowMut;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

pub const LENGTH_VIEW_DATA: usize = 500;
pub const WINDOW_WIDTH: u32 = 800;
pub const WINDOW_LENGTH: u32 = 600;

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

pub fn data_view(receiver_nav_view: Receiver<NavigationView>) {
    let fps_max = 6;
    let mut window: PistonWindow =
        WindowSettings::new("Realtime GNSS-SDR-RS", [WINDOW_WIDTH, WINDOW_LENGTH])
            //.samples(4)
            .exit_on_esc(true)
            .build()
            .unwrap();

    window.set_max_fps(fps_max);

    let mut sat_visibility: Vec<f32> = vec![0.0; 32];
    let mut i_p: Vec<f32> = Vec::new();
    let mut q_p: Vec<f32> = Vec::new();
    for i in 1..=32 {
        sat_visibility.insert(i, 0.0);
    }

    while let Some(e) = window.next() {
        if let Ok(nav_view) = receiver_nav_view.recv() {
            sat_visibility[nav_view.prn] = nav_view.acq_mag;
            i_p = nav_view.trk_I_P.into();
            q_p = nav_view.trk_Q_P.into();
        }

        let mut mags: Vec<f32> = sat_visibility.clone();
        mags.push(5.0);
        let (max_mags, _) = max_float_vec(mags).unwrap();
        /*window.draw_2d(&e, |c, g, _| {
            clear([1.0; 4], g);
            let drawing_area =
                plotters::coord::types::RangedCoordf32::axis_pixel_range(&self, limit);
        });
        */
        let mut buf = vec![0u8; WINDOW_WIDTH as usize * WINDOW_LENGTH as usize];
        let root = BitMapBackend::<RGBPixel>::with_buffer_and_format(
            buf.borrow_mut(),
            (WINDOW_WIDTH, WINDOW_LENGTH),
        )
        .unwrap()
        .into_drawing_area();
        root.fill(&WHITE).unwrap();
        let mut chart1 = ChartBuilder::on(&root)
            .build_cartesian_2d(0.0..32.0_f32, 0.0..2.0 * max_mags)
            .unwrap();

        /*chart1
            .draw_series(sat_visibility.iter().enumerate().map(|(x, &y)| {
                Rectangle::new(
                    [((x + 1) as f32 - 0.5, 0.0), ((x + 1) as f32 + 0.5, y)],
                    BLUE.filled(),
                )
            }))
            .unwrap();
        */
        let (max_i_p, _) = max_float_vec(i_p.clone().into()).unwrap();
        let mut chart2 = ChartBuilder::on(&root)
            .build_cartesian_2d(0..LENGTH_VIEW_DATA, 0.0..2.0 * max_i_p)
            .unwrap();

        chart2
            .draw_series(LineSeries::new(
                (0..).zip(i_p.iter()).map(|(i, &y)| (i, y)),
                &BLUE,
            ))
            .unwrap();

        let (max_q_p, _) = max_float_vec(q_p.clone().into()).unwrap();
        let mut chart3 = ChartBuilder::on(&root)
            .build_cartesian_2d(0..LENGTH_VIEW_DATA, 0.0..2.0 * max_q_p)
            .unwrap();

        chart3
            .draw_series(LineSeries::new(
                (0..).zip(q_p.iter()).map(|(i, &y)| (i, y)),
                &BLUE,
            ))
            .unwrap();
    }
}
