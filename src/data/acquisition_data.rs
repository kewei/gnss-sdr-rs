
#[derive(Debug, Clone)]
pub struct AcquisitionData {
    pub prn: u8,
    pub fft_power: Vec<f32>,
    pub doppler_hz: f32,
    pub code_phase: usize,
}

impl AcquisitionData {
    pub fn new() -> Self {
        Self {
            prn: 0,
            fft_power: Vec::new(),
            doppler_hz: 0.0,
            code_phase: 0,
        }
    }
}