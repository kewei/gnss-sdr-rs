use crate::acquisition::do_acquisition::{AcquisitionResult, ChannelState};
use crate::constants::gps_ca_constants::GPS_CA_CODE_32_PRN;
use crate::constants::gps_property_constants::{
    GPS_L1_CA_CODE_LENGTH_CHIPS, GPS_L1_CA_CODE_RATE_CHIPS_PER_S,
};
use crate::utilities::ca_code::generate_ca_code_samples;
use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
use crossbeam_channel::{Receiver, Sender};
use num_complex::Complex32;
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use std::error::Error;
use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::PoisonError;

const LOCK_THRESHOLD: f32 = 15.0;
const MAX_LOST_EPOCHS: u32 = 20; // ms
const NUM_OF_CHANNELS: usize = 15;
static DLL_DUMPING_RATIO: f32 = 0.7;
static PLL_DUMPING_RATIO: f32 = 0.7;
static PLL_GAIN: f32 = 0.25;
static DLL_NOISE_BANDWIDTH: f32 = 2.0;
static PLL_NOISE_BANDWIDTH: f32 = 25.0;
static DLL_GAIN: f32 = 1.0;
// Summation interval
static PLL_SUM_CARR: f32 = 0.001;
static DLL_SUM_CODE: f32 = 0.001;
static EARLY_LATE_SPACE: f32 = 0.5;
pub static LOOP_MS: usize = 10;

#[derive(Debug, Clone)]
pub struct TrackingError;
impl std::fmt::Display for TrackingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrackingError")
    }
}

impl Error for TrackingError {}

impl<T> From<PoisonError<T>> for TrackingError {
    fn from(_: PoisonError<T>) -> Self {
        TrackingError
    }
}

pub enum TrackingMessage {
    SatelliteLost(u8),
    SatelliteLocked(u8),
}

pub struct LoopFilter {
    pub tau1: f32,
    pub tau2: f32,
}

impl LoopFilter {
    /// PLL Bandwidth: Usually 15Hz to 25Hz,  can't track movement/vibration <-narrow----wide-> track noise and lose lock
    /// DLL Bandwidth: Usually 0.5Hz to 2Hz, much slower than the phase loop
    pub fn new(noise_bw: f32, dumping_ratio: f32, gain: f32) -> Self {
        let w = noise_bw * 8.0 * dumping_ratio / (4.0 * dumping_ratio.powf(2.0) + 1.0);
        let tau1 = gain / (w * w);
        let tau2 = (2.0 * dumping_ratio) / w;
        Self { tau1, tau2 }
    }

    #[inline(always)]
    pub fn update(&mut self, d_err: f32, err: f32, dt: f32) -> f32 {
        d_err * (dt / self.tau1) + (d_err - err) * (self.tau2 / self.tau1)
    }
}

// pub struct CacodeTable {
//     pub samples: Vec<Vec<i8>>,
// }

// impl CacodeTable {
//     pub fn new(prn: u8, fs:f32) -> Self {
//         Self{
//             samples: generate_ca_code_samples(prn, fs)
//         }
//     }
// }
// pub struct CarrierNco{

// }

pub struct TrackingChannel {
    pub id: u8,
    pub prn: u8,
    pub state: ChannelState,
    pub lost_counter: u32,
    pub fs: f32,
    pub next_sample_index: usize,
    pub num_samples_per_code: usize,
    pub ca_code_samples: Vec<i8>,
    pub data_samples: Vec<Complex32>,

    pub carrier_freq: f32,
    pub carrier_phase: f32,
    pub carrier_error: f32,
    pub carrier_nco: f32,
    pub code_phase: f32,
    pub code_error: f32,
    pub code_nco: f32,
    pub code_rate: f32,
    pub cos_p: Vec<f32>,
    pub sin_p: Vec<f32>,

    pub i_prompt: f32,
    pub q_prompt: f32,

    pub pll_filter: LoopFilter,
    pub dll_filter: LoopFilter,
}

impl TrackingChannel {
    pub fn new(id: u8, fs: f32) -> Self {
        let num_ca_samples =
            (fs / (GPS_L1_CA_CODE_RATE_CHIPS_PER_S / GPS_L1_CA_CODE_LENGTH_CHIPS)).round() as usize;
        Self {
            id,
            prn: 0,
            state: ChannelState::Idle,
            lost_counter: 0,
            next_sample_index: 0,
            num_samples_per_code: num_ca_samples,
            ca_code_samples: vec![0; (1.5 * num_ca_samples as f32).round() as usize], // pre-allocate more samples to avoid frequent resizing during tracking
            data_samples: vec![
                Complex32::new(0.0, 0.0);
                (1.5 * num_ca_samples as f32).round() as usize
            ],
            fs: fs,
            carrier_freq: 0.0,
            carrier_phase: 0.0,
            carrier_error: 0.0,
            carrier_nco: 0.0,
            code_phase: 0.0,
            code_error: 0.0,
            code_nco: 0.0,
            code_rate: GPS_L1_CA_CODE_RATE_CHIPS_PER_S,
            cos_p: vec![0.0; (1.5 * num_ca_samples as f32).round() as usize],
            sin_p: vec![0.0; (1.5 * num_ca_samples as f32).round() as usize],
            i_prompt: 0.0,
            q_prompt: 0.0,
            pll_filter: LoopFilter::new(PLL_NOISE_BANDWIDTH, PLL_DUMPING_RATIO, PLL_GAIN),
            dll_filter: LoopFilter::new(DLL_NOISE_BANDWIDTH, DLL_DUMPING_RATIO, DLL_GAIN),
        }
    }

    pub fn start(&mut self, result: AcquisitionResult) {
        self.ca_code_samples = generate_ca_code_samples(result.prn, self.code_rate, self.fs);
        self.prn = result.prn;
        self.carrier_freq = result.carrier_freq;
        self.code_phase = result.code_phase_chips;
        self.state = ChannelState::Tracking(result.prn);
    }

    pub fn is_active(&self) -> bool {
        self.state == ChannelState::Tracking(self.prn)
    }

    pub fn update(&mut self, buff: Arc<MulticastRingBuffer>) -> Option<TrackingMessage> {
        if self.state != ChannelState::Tracking(self.prn) {
            return None;
        }

        self.ca_code_samples = generate_ca_code_samples(self.prn, self.code_rate, self.fs);
        self.num_samples_per_code = self.ca_code_samples.len();

        let head = buff.get_head();

        if (head.wrapping_sub(self.next_sample_index + self.num_samples_per_code) as isize) < 0 {
            return None;
        }

        buff.copy_to_slice(
            self.next_sample_index,
            &mut self.data_samples[0..self.num_samples_per_code],
        );

        self.do_work()
    }

    fn do_work(&mut self) -> Option<TrackingMessage> {
        let (i_p, q_p, i_e, q_e, i_l, q_l) = self.early_late_correlation();

        let power = i_p * i_p + q_p * q_p;

        if power > LOCK_THRESHOLD {
            self.lost_counter = 0;
            self.run_loop_filters(i_p, q_p, i_e, q_e, i_l, q_l);
            self.next_sample_index += self.num_samples_per_code;
            self.num_samples_per_code =
                (self.fs / (self.code_rate / GPS_L1_CA_CODE_LENGTH_CHIPS)).round() as usize; // Used to calculate the next sample index in TrackingManger
            None
        } else {
            self.lost_counter += 1;
            if self.lost_counter >= MAX_LOST_EPOCHS {
                self.reset();
                Some(TrackingMessage::SatelliteLost(self.prn))
            } else {
                self.next_sample_index += self.num_samples_per_code;
                self.num_samples_per_code =
                    (self.fs / (self.code_rate / GPS_L1_CA_CODE_LENGTH_CHIPS)).round() as usize;
                None
            }
        }
    }
    
    // pub fn get_phases_lut(&mut self) {
    //     for i in 0..self.num_samples_per_code {
    //         let phase = self.carrier_phase + (2.0 * PI * self.carrier_freq * (i as f32) / self.fs);
    //         self.cos_p[i] = phase.cos();
    //         self.sin_p[i] = -phase.sin();
    //     }
    // }

    // pub fn get_ca_code_lut(&self) -> Vec<f32> {

    // }

    // pub fn early_late_correlation_simd(&mut self) {
    //     for (samples_chunk, cos_chunk, sin_chunk, ca_chunk) in self.data_samples.chunks(8).zip(self.cos_p.chunks(8)).zip(self.sin_p.chunks(8)).zip(self.ca_code_samples.chunks(8)) {
    //         let chunk_real = f32x8::from_slice(&samples_chunk.iter().map(|c| c.re).collect::<Vec<_>>());
    //         let chunk_img = f32x8::from_slice(&samples_chunk.iter().map(|c| c.im).collect::<Vec<_>>());
    //     }
    // }

    pub fn early_late_correlation(&mut self) -> (f32, f32, f32, f32, f32, f32) {
        for i in 0..self.num_samples_per_code {
            let phase = self.carrier_phase + (2.0 * PI * self.carrier_freq * (i as f32) / self.fs);
            let cos_p = phase.cos();
            let sin_p = -phase.sin();

            self.data_samples[i] = self.data_samples[i] * Complex32::new(cos_p, sin_p);
        }

        self.carrier_phase = (self.carrier_phase
            + 2.0 * PI * self.carrier_freq * (self.num_samples_per_code as f32 / self.fs))
            % (2.0 * PI);

        let mut i_p = 0.0_f32;
        let mut q_p = 0.0_f32;
        let mut i_e = 0.0_f32;
        let mut q_e = 0.0_f32;
        let mut i_l = 0.0_f32;
        let mut q_l = 0.0_f32;

        for i in 0..self.num_samples_per_code {
            let chip_idx = (self.code_phase + (i as f32 * (self.code_rate / self.fs))) % 1023.0;
            let p_chip = self.get_ca_chip(chip_idx);
            let e_chip = self.get_ca_chip(chip_idx + EARLY_LATE_SPACE);
            let l_chip = self.get_ca_chip(chip_idx - EARLY_LATE_SPACE);

            i_p += self.data_samples[i].re * p_chip;
            q_p += self.data_samples[i].im * p_chip;
            i_e += self.data_samples[i].re * e_chip;
            q_e += self.data_samples[i].im * e_chip;
            i_l += self.data_samples[i].re * l_chip;
            q_l += self.data_samples[i].im * l_chip;
        }

        self.code_phase = (self.code_phase
            + (self.code_rate / self.fs) * (self.num_samples_per_code as f32))
            % 1023.0;

        (i_p, q_p, i_e, q_e, i_l, q_l)
    }

    pub fn get_ca_chip(&self, phase: f32) -> f32 {
        let idx = (phase.floor() as usize) % 1023;
        GPS_CA_CODE_32_PRN[self.prn as usize][idx] as f32
    }

    pub fn run_loop_filters(&mut self, i_p: f32, q_p: f32, i_e: f32, q_e: f32, i_l: f32, q_l: f32) {
        let pll_err = (q_p / i_p).atan() / (2.0 * PI);
        self.carrier_nco = self
            .pll_filter
            .update(pll_err, self.carrier_error, PLL_SUM_CARR);
        self.carrier_error = pll_err;
        self.carrier_freq += self.carrier_nco; // with positive doppler, the local carrier needs to add a 
        // NCO to catch up

        let pow_e = (i_e.powi(2) + q_e.powi(2)).sqrt();
        let pow_l = (i_l.powi(2) + q_l.powi(2)).sqrt();

        let dll_err = if (pow_e + pow_l) != 0.0 {
            (pow_e - pow_l) / (pow_e + pow_l)
        } else {
            0.0
        };

        self.code_nco = self
            .dll_filter
            .update(dll_err, self.code_error, DLL_SUM_CODE);
        self.code_error = dll_err;
        self.code_rate += self.code_nco; // If the signal hits i_e harder, nco is positive.
    }

    pub fn reset(&mut self) {
        self.prn = 0;
        self.state = ChannelState::Idle;
        self.lost_counter = 0;
        self.next_sample_index = 0;
        self.carrier_freq = 0.0;
        self.carrier_phase = 0.0;
        self.carrier_error = 0.0;
        self.carrier_nco = 0.0;
        self.code_phase = 0.0;
        self.code_error = 0.0;
        self.code_nco = 0.0;
        self.code_rate = 0.0;
        self.i_prompt = 0.0;
        self.q_prompt = 0.0;
    }
}

pub struct TrackingManager {
    pub channels: Vec<TrackingChannel>,
    pub acq_to_trk: Receiver<AcquisitionResult>,
    pub trk_to_acq: Sender<TrackingMessage>,
}

impl TrackingManager {
    pub fn new(
        acq_to_trk: Receiver<AcquisitionResult>,
        trk_to_acq: Sender<TrackingMessage>,
        fs: f32,
    ) -> Self {
        Self {
            channels: (0..NUM_OF_CHANNELS)
                .map(|id| TrackingChannel::new(id as u8, fs))
                .collect(),
            acq_to_trk,
            trk_to_acq,
        }
    }

    pub fn process_channels(&mut self, multi_ring_buf: Arc<MulticastRingBuffer>) {
        while let Ok(msg) = self.acq_to_trk.try_recv() {
            if let Some(channel) = self
                .channels
                .iter_mut()
                .find(|c| c.state == ChannelState::Idle)
            {
                let _ = self
                    .trk_to_acq
                    .send(TrackingMessage::SatelliteLocked(msg.prn));
                channel.start(msg);
            }
        }

        self.channels
            .par_iter_mut()
            .filter(|c| c.is_active())
            .for_each(|chnl| {
                if let Some(msg) = chnl.update(multi_ring_buf.clone()) {
                    let _ = self.trk_to_acq.send(msg);
                }
            });
    }

    fn next_tracking_index(&self) -> usize {
        self.channels
            .iter()
            .filter(|c| c.is_active())
            .map(|c| c.next_sample_index + c.num_samples_per_code)
            .min()
            .unwrap_or(0)
    }
}

pub fn run(
    multi_ring_buf: Arc<MulticastRingBuffer>,
    acq_to_trk: Receiver<AcquisitionResult>,
    trk_to_acq: Sender<TrackingMessage>,
    fs: f32,
) -> Result<(), TrackingError> {
    let mut manager = TrackingManager::new(acq_to_trk, trk_to_acq, fs);
    loop {
        let mut curr_head = multi_ring_buf.get_head();
        let mut required_idx = manager.next_tracking_index();
        if (curr_head.wrapping_sub(required_idx) as isize) < 0 {
            let mut head_guard = multi_ring_buf.notifier.lock()?;
            while (multi_ring_buf
                .get_head()
                .wrapping_sub(manager.next_tracking_index()) as isize)
                < 0
            {
                head_guard = multi_ring_buf.condvar.wait(head_guard)?;
            }

            curr_head = multi_ring_buf.get_head();
            drop(head_guard);
        }

        while (curr_head.wrapping_sub(required_idx) as isize) >= 0 {
            manager.process_channels(multi_ring_buf.clone());

            required_idx = manager.next_tracking_index();
            curr_head = multi_ring_buf.get_head();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::do_acquisition::AcquisitionResult;
    use crate::acquisition::{do_acquisition, doppler_shift};
    use crate::constants::gps_property_constants;
    use crate::tracking::do_tracking::TrackingChannel;
    use crate::utilities::ca_code;
    use num_complex::Complex32;
    use std::f32::consts::PI;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use std::time::Instant;

    /// Helper to generate 1ms of synthetic GPS L1 data
    fn generate_synthetic_signal(
        ca_code: &[i8],
        doppler: f32,
        starting_carrier_phase: f32,
        starting_code_phase: f32,
        f_sampling: f32,
    ) -> Vec<Complex32> {
        let samples_per_ms = (f_sampling / 1000.0) as usize;
        let mut samples = Vec::with_capacity(samples_per_ms);
        let code_phase_step = gps_property_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / f_sampling;

        for i in 0..samples_per_ms {
            // 1. Calculate continuous carrier phase
            let carrier_phase =
                starting_carrier_phase + (2.0 * PI * doppler / f_sampling * i as f32);

            // 2. Calculate code phase and lookup chip
            let current_code_phase = starting_code_phase + (code_phase_step * i as f32);
            let chip_idx = (current_code_phase.floor() as usize) % 1023;
            let code_val = ca_code[chip_idx] as f32;

            // 3. Synthesize the complex sample (Code * Carrier)
            samples.push(Complex32::new(
                code_val * carrier_phase.cos(),
                code_val * carrier_phase.sin(),
            ));
        }
        samples
    }

    #[test]
    fn test_pll_frequency_pull_in() {
        let prn = 2;
        let f_sampling = 4_096_000.0;
        let mock_ca_code =
            ca_code::generate_ca_code_samples(prn, GPS_L1_CA_CODE_RATE_CHIPS_PER_S, f_sampling);

        let true_doppler = 3000.0;
        let signal_samples =
            generate_synthetic_signal(&mock_ca_code, true_doppler, 0.0, 0.0, f_sampling);

        let buf = Arc::new(MulticastRingBuffer::new(8 * signal_samples.len()));
        let _ = buf.write_samples(&signal_samples);

        assert_eq!(buf.get_head(), signal_samples.len());

        let mut trk_chl = TrackingChannel::new(0, f_sampling);
        trk_chl.start(AcquisitionResult {
            prn: prn,
            carrier_freq: 2950.0, // We start with a local carrier that is 50 Hz slower than the true signal
            code_phase_samples: 0,
            code_phase_chips: 0.0,
            fs: f_sampling,
            mag_relative: 10.0,
            sample_global_index: 0,
        });

        let now = Instant::now();
        trk_chl.update(buf.clone());
        println!("Time for first update: {:?}", now.elapsed());

        let err1 = trk_chl.carrier_error;
        // let nco1 = trk_chl.carrier_nco;
        // let freq1 = trk_chl.carrier_freq;

        // println!("Carrier error after first update: {}", err1);
        // println!("Carrier NCO after first update: {}", nco1);
        // println!("Carrier frequency after first update: {}", freq1);

        assert!(
            trk_chl.carrier_error > 0.0,
            "Discriminator failed: Expected a positive phase error, got {}",
            trk_chl.carrier_error
        );

        assert!(
            trk_chl.carrier_nco > 0.0,
            "Filter failed: Expected positive NCO push to speed up the local carrier, got {}",
            trk_chl.carrier_nco
        );

        assert!(
            trk_chl.carrier_freq > 2950.0,
            "State update failed: Frequency did not adjust upward. New freq: {}",
            trk_chl.carrier_freq
        );

        let _ = buf.write_samples(&signal_samples);
        let _ = buf.write_samples(&signal_samples);

        assert_eq!(buf.get_head(), 3 * signal_samples.len());
        assert_eq!(trk_chl.next_sample_index, trk_chl.num_samples_per_code);

        let samplers_per_code = trk_chl.num_samples_per_code;

        let now = Instant::now();
        trk_chl.update(buf.clone());
        println!("Time for second update: {:?}", now.elapsed());

        let err2 = trk_chl.carrier_error;
        // let nco2 = trk_chl.carrier_nco;
        // let freq2 = trk_chl.carrier_freq;

        // println!("Carrier error after second update: {}", err2);
        // println!("Carrier NCO after second update: {}", nco2);
        // println!("Carrier frequency after second update: {}", freq2);
        // println!("Samples per code: {}", trk_chl.num_samples_per_code);

        assert!((true_doppler - err2).abs() < (true_doppler - err1).abs());

        let _ = buf.write_samples(&signal_samples);

        assert_eq!(buf.get_head(), 4 * signal_samples.len());
        assert_eq!(
            trk_chl.next_sample_index,
            samplers_per_code + trk_chl.num_samples_per_code
        );

        let samplers_per_code = trk_chl.next_sample_index;

        trk_chl.update(buf.clone());

        let err3 = trk_chl.carrier_error;
        // let nco3 = trk_chl.carrier_nco;
        // let freq3 = trk_chl.carrier_freq;

        // println!("Carrier error after third update: {}", err3);
        // println!("Carrier NCO after third update: {}", nco3);
        // println!("Carrier frequency after third update: {}", freq3);
        // println!("Samples per code: {}", trk_chl.num_samples_per_code);

        assert_eq!(
            trk_chl.next_sample_index,
            samplers_per_code + trk_chl.num_samples_per_code
        );
        assert!((true_doppler - err3).abs() < (true_doppler - err2).abs());
    }

    #[test]
    fn test_dll_code_phase_tracking() {
        let f_sampling = 4_096_000.0;
        let prn = 3;
        let mock_ca_code =
            ca_code::generate_ca_code_samples(prn, GPS_L1_CA_CODE_RATE_CHIPS_PER_S, f_sampling);

        let signal_samples = generate_synthetic_signal(&mock_ca_code, 0.0, 0.0, 0.25, f_sampling);

        let buf = Arc::new(MulticastRingBuffer::new(2 * signal_samples.len()));
        let _ = buf.write_samples(&signal_samples);

        assert_eq!(buf.get_head(), signal_samples.len());

        let mut trk_chl = TrackingChannel::new(prn, f_sampling);
        trk_chl.start(AcquisitionResult {
            prn: prn,
            carrier_freq: 0.0,
            code_phase_samples: 0,
            code_phase_chips: 0.0, // Our local code starts perfectly aligned, but the real signal is early
            fs: f_sampling,
            mag_relative: 10.0,
            sample_global_index: 0,
        });

        let now = Instant::now();
        trk_chl.update(buf.clone());
        println!("Time for first update: {:?}", now.elapsed());

        // let err1 = trk_chl.code_error;
        // let nco1 = trk_chl.code_nco;
        // let freq1 = trk_chl.code_rate;

        // println!("Code error after first update: {}", err1);
        // println!("Code NCO after first update: {}", nco1);
        // println!("Code rate after first update: {}", freq1);

        let _ = buf.write_samples(&signal_samples);
        let _ = buf.write_samples(&signal_samples);

        assert_eq!(buf.get_head(), 3 * signal_samples.len());
        assert_eq!(trk_chl.next_sample_index, trk_chl.num_samples_per_code);

        let samplers_per_code = trk_chl.num_samples_per_code;

        let now = Instant::now();
        trk_chl.update(buf.clone());
        println!("Time for second update: {:?}", now.elapsed());

        // let err2 = trk_chl.code_error;
        // let nco2 = trk_chl.code_nco;
        // let freq2 = trk_chl.code_rate;

        // println!("Code error after second update: {}", err2);
        // println!("Code NCO after second update: {}", nco2);
        // println!("Code rate after second update: {}", freq2);
        // println!("Samples per code: {}", trk_chl.num_samples_per_code);

        let _ = buf.write_samples(&signal_samples);

        assert_eq!(buf.get_head(), 4 * signal_samples.len());
        assert_eq!(
            trk_chl.next_sample_index,
            samplers_per_code + trk_chl.num_samples_per_code
        );

        let samplers_per_code = trk_chl.next_sample_index;

        trk_chl.update(buf.clone());

        // let err3 = trk_chl.code_error;
        // let nco3 = trk_chl.code_nco;
        // let freq3 = trk_chl.code_rate;

        // println!("Code error after third update: {}", err3);
        // println!("Code NCO after third update: {}", nco3);
        // println!("Code rate after third update: {}", freq3);
        // println!("Samples per code: {}", trk_chl.num_samples_per_code);

        assert_eq!(
            trk_chl.next_sample_index,
            samplers_per_code + trk_chl.num_samples_per_code
        );
    }

    #[test]
    fn test_tracking_with_real_signal() {
        const FS: f32 = 16_367_600.0;
        const IF: f32 = 4_130_400.0;
        const NUM_INTEGRATIONS: usize = 10;
        const MS_SAMPLES: usize = NUM_INTEGRATIONS * 16368;

        let root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(root)
            .join("src")
            .join("test_data")
            .join("GPS_recordings")
            .join("gioveAandB_short.bin");

        let mut file = match File::open(file_path) {
            Ok(f) => f,
            Err(_) => {
                println!("Raw data file not found. Skipping real-data test.");
                return;
            }
        };

        let mut bytes = vec![0u8; MS_SAMPLES * NUM_INTEGRATIONS];
        file.read_exact(&mut bytes)
            .expect("Failed to read raw data file");

        let buffer = bytes
            .iter()
            .map(|b| Complex32::new((*b as i8) as f32, 0.0))
            .collect::<Vec<Complex32>>();

        let doppler_start = -7000.0;
        let doppler_end = 7000.0;
        let step = 500.0;
        let mut doppler_tables = Vec::new();
        let mut current_doppler = doppler_start;

        while current_doppler <= doppler_end {
            doppler_tables.push(doppler_shift::DopplerShiftTable::new(
                IF,
                current_doppler,
                FS,
                MS_SAMPLES / NUM_INTEGRATIONS,
            ));
            current_doppler += step;
        }

        let prn = 6;
        let mut acq_worker =
            do_acquisition::AcquisitionWorker::new(prn, MS_SAMPLES / NUM_INTEGRATIONS, FS);
        let aqc_result = acq_worker
            .search_satellite(&buffer, &doppler_tables, 0, NUM_INTEGRATIONS)
            .expect("Failed to acquire satellite");

        let mut trk_channel = TrackingChannel::new(0, FS);
        trk_channel.start(aqc_result);

        let mut prompt_i = Vec::new();
        let mut doppler_history = Vec::new();

        for _ in 0..500 {
            trk_channel.update(buff)
        }
    }
}
