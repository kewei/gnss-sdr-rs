use crate::acquisition::doppler_shift::{DopplerShiftTable, apply_doppler_shift};
use crate::utilities::ca_code::generate_ca_code_samples;
use num::Complex;
use rayon::prelude::*;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

const FFT_LENGTH_MS: u8 = 1;
const FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
const FREQ_SEARCH_STEP_HZ: u16 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: u8 = 32; // 32 PRN codes to search
const LONG_SAMPLES_LENGTH: u8 = 11; // ms

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
    pub prn: u8,
    pub code_phase: usize,
    pub carrier_freq: f32,
    pub mag_relative: f32,
}

impl AcquisitionResult {
    pub fn new(prn: u8) -> Self {
        Self {
            prn,
            code_phase: 0,
            carrier_freq: 0.0,
            mag_relative: 0.0,
        }
    }
}

pub enum ChannelState {
    Idle,
    Acquiring,
    Tracking,
}

struct AcquisitionManager {
    pub active_prns: Arc<Mutex<HashSet<u8>>>,
    pub last_search_time: std::time::Instant,
    pub acquisition_interval: u16,
}

impl AcquisitionManager {
    pub fn new(acquisition_interval: u16) -> Self {
        Self {
            active_prns: Arc::new(Mutex::new(HashSet::new())),
            last_search_time: std::time::Instant::now(),
            acquisition_interval,
        }
    }
    
    pub fn get_search_list(&self) -> Vec<u8> {
        let active = self.active_prns.lock()?;
        (1..=PRN_SEARCH_ACQUISITION_TOTAL as u8)
            .filter(|prn| !active.contains(prn))
            .collect()
    }
}

pub struct AcquisitionWorker {
    prn: u8,
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    fft_size: usize,
    freq_sampling_hz: f32,
    // doppler_table: &DopplerShiftTable,
    ca_code_samples_fft: &[Complex<f32>],
    result_buf: Vec<Complex<f32>>,
    freq_replica: Vec<Complex<f32>>,
}

impl AcquisitionWorker {
    fn new(prn: u8, fft_size: usize, freq_sampling_hz: f32) -> Self {
        let mut planner = FftPlanner::new();
        // let capacity = (FREQ_SEARCH_ACQUISITION_HZ / FREQ_SEARCH_STEP_HZ) as usize + 1;
        // let mut doppler_table = Vec::with_capacity(capacity);
        // for i in 0..capacity {
        //     let doppler_freq = -FREQ_SEARCH_ACQUISITION_HZ / 2.0 + i as f32 * FREQ_SEARCH_STEP_HZ as f32;
        //     doppler_table.push(DopplerShiftTable::new(doppler_freq, freq_sampling_hz, fft_size));
        // }

        // let mut ca_code_samples = Vec::with_capacity(PRN_SEARCH_ACQUISITION_TOTAL);
        // for prn in 1..=PRN_SEARCH_ACQUISITION_TOTAL {
        //     let samples = generate_ca_code_samples(prn, freq_sampling_hz);
        //     ca_code_samples.push(samples);
        // }

        // let mut ca_code_samples_fft = [[Complex::new(0.0, 0.0); ca_code_samples[0].len()]; PRN_SEARCH_ACQUISITION_TOTAL];
        // for prn in 1..=PRN_SEARCH_ACQUISITION_TOTAL {
        //     // Convert to complex and do FFT
        //     ca_code_samples_fft[prn] = ca_code_samples[prn - 1].iter().map(|&s| Complex::new(s as f32, 0.0)).collect();
        //     planner.plan_fft_forward(fft_size).process(&mut ca_code_samples_fft[prn]);
        // }

        let ca_samples = generate_ca_code_samples(prn, freq_sampling_hz);
        let mut ca_code_samples_fft = [Complex::new(0.0, 0.0); ca_code_samples.len()];
        planner
            .plan_fft_forward(fft_size)
            .process(&mut ca_code_samples_fft);

        Self {
            prn: prn,
            fft: planner.plan_fft_forward(fft_size),
            ifft: planner.plan_fft_inverse(fft_size),
            fft_size: fft_size,
            freq_sampling_hz: freq_sampling_hz,
            // doppler_table: doppler_table.as_slice(),
            ca_code_samples_fft: &ca_code_samples_fft,
            result_buf: vec![Complex::new(0.0, 0.0); fft_size],
            freq_replica: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    fn search_satellite(
        &mut self,
        samples_chunk: &[Complex<f32>],
        doppler_table: &[DopplerShiftTable],
    ) -> Option<AcquisitionResult> {
        let mut max_val: f32 = 0.0;
        let mut best_doper_freq: f32 = 0.0;
        let mut best_code_phase: usize = 0;
        let mut power_results = vec![0.0; self.fft_size];

        for doppler in ((-FREQ_SEARCH_ACQUISITION_HZ / 2)..=(FREQ_SEARCH_ACQUISITION_HZ / 2))
            .step_by(FREQ_SEARCH_STEP_HZ)
        {
            apply_doppler_shift(
                samples_chunk,
                &doppler_table[doppler as usize],
                &mut self.result_buf,
            );
            self.fft.process(&mut self.result_buf);

            for i in 0..self.fft_size {
                self.result_buf[i] *= self.ca_code_samples_fft[i].conj();
            }

            self.ifft.process(&mut self.result_buf);

            for (idx, val) in self.result_buf.iter().enumerate() {
                let mag = val.norm_sqr();
                power_results[idx] = mag;
                if mag > max_val {
                    max_val = mag;
                    best_doper_freq = doppler;
                    best_code_phase = idx;
                }
            }

            if self.is_good_satellite(&power_results, max_val) {
                return Some(AcquisitionResult {
                    prn: self.prn,
                    code_phase: best_code_phase,
                    carrier_freq: best_doper_freq,
                    mag_relative: max_val,
                });
            }
        }

        return None;
    }

    // SIMD sum
    fn is_good_satellite(&self, power_results: &[f32], max_val: f32) -> bool {
        let sum_power = power_results
            .chunks_exact(8)
            .map(|chunk| f32x8::from_slice(chunk))
            .fold(f32x8::splat(0.0), |acc, x| acc + x)
            .reduce_sum();
        let avg_power: f32 = (sum_power - max_val) / (self.fft_size - 1) as f32;

        max_val / avg_power > 7.0
    }
}

pub fn do_acquisition(
    multi_buffer: Arc<MulticastRingBuffer>,
    mut acquisition_worker: AcquisitionWorker,
    freq_sampling_hz: f32,
) {
    let manager = AcquisitionManager::new(500);
    let capacity = (FREQ_SEARCH_ACQUISITION_HZ / FREQ_SEARCH_STEP_HZ) as usize + 1;
    let mut doppler_table = Vec::with_capacity(capacity);
    for i in 0..capacity {
        let doppler_freq =
            -FREQ_SEARCH_ACQUISITION_HZ / 2.0 + i as f32 * FREQ_SEARCH_STEP_HZ as f32;
        doppler_table.push(DopplerShiftTable::new(
            doppler_freq,
            freq_sampling_hz,
            fft_size,
        ));
    }
    let fft_size = (freq_sampling_hz
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let mut workers = (1..=PRN_SEARCH_ACQUISITION_TOTAL)
        .par_iter()
        .filter_map(|&prn| AcquisitionWorker::new(prn, fft_size, freq_sampling_hz))
        .collect::<Vec<AcquisitionWorker>>();

    let mut local_tail = 0;
    let mut chunk_samples = vec![Complex::new(0.0, 0.0); fft_size];

    loop {
        let now = std::time::Instant::now();
        // Acquisition interval: 500ms
        if now.duration_since(manager.last_search_time).as_millis() < manager.acquisition_interval as u128 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        let head = multi_buffer.get_head();
        if head >= local_tail + fft_size {
            let search_list = manager.get_search_list();

            if search_list.is_empty() {
                // All satellites are in tracking state, sleep long: 1s
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }

            multi_buffer.copy_to_slice(local_tail, &mut chunk_samples);

            let results: Vec<AcquisitionResult> = workers
                .par_iter_mut()
                .filter_map(|worker| worker.search_satellite(&chunk_samples, &doppler_table))
                .collect();

            for result in results {
                manager.active_prns.lock()?.insert(result.prn);
                tracking_worker.spawn_tracking_thread(result);
            }

            manager.last_search_time = std::time::Instant::now();

            local_tail += fft_size;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
