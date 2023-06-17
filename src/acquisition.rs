use rustfft::{num_complex::Complex, FftPlanner};
use std::error::Error;

pub struct AcquistionStatistics {}

pub fn do_acquisition(
    samples: Vec<u8>,
    fre_sampling: u32,
) -> Result<AcquistionStatistics, Box<dyn Error>> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(4096);
    let mut buffer = vec![
        Complex {
            re: 0.0f32,
            im: 0.0f32
        };
        4096
    ];
    fft.process(&mut buffer);
    todo!()
}
