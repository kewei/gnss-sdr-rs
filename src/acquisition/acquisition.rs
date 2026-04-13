const FFT_LENGTH_MS: usize = 1;
const FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
const FREQ_SEARCH_STEP_HZ: i32 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: usize = 32; // 32 PRN codes to search
const LONG_SAMPLES_LENGTH: i8 = 11; // ms

#[derive(Debug, Clone)]
struct AcqError;

impl fmt::Display for AcqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error happens while doing signal acquisition!")
    }
}

impl Error for AcqError {}

#[derive(Debug, Clone)]
pub struct AcquisitionResult {
    pub prn: usize,
    pub code_phase: usize,
    pub carrier_freq: f32,
    pub mag_relative: f32,
    pub ca_code: Vec<i16>,
    pub ca_code_samples: Vec<i16>,
    pub cn0: f32,
}

impl AcquisitionResult {
    pub fn new(prn: usize, f_sampling: f32) -> Self {
        let (ca_code_samples, ca_code) = generate_ca_code_samples(prn, f_sampling);
        Self {
            prn,
            code_phase: 0,
            carrier_freq: 0.0,
            mag_relative: 0.0,
            ca_code,
            ca_code_samples,
            cn0: 0.0,
        }
    }
}

pub enum ChannelState {
    Idle,
    Acquiring,
    Tracking,
}

pub fn do_acquisition(
    acquisition_result: Arc<Mutex<AcquisitionResult>>,
    freq_sampling: f32,
    freq_IF: f32,
    is_complex: bool,
) -> Result<usize, &'static str> {}