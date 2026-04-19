use crate::acquisition::doppler_shift::{DopplerShiftTable, apply_doppler_shift};
use crate::utilities::ca_code::generate_ca_code_samples;
use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
use crate::constants::gps_def_constants::{GPS_L1_CA_CODE_RATE_CHIPS_PER_S, GPS_L1_CA_CODE_LENGTH_CHIPS};
use crate::tracking::tracking::TrackingManager;
use num::Complex;
use rayon::prelude::*;
use rustfft::{Fft, FftPlanner};
use std::sync::{Arc, Mutex, PoisonError};
use std::fmt;
use std::error::Error;
use std::collections::HashSet;
use std::simd::f32x8;
use std::simd::num::SimdFloat;

const FFT_LENGTH_MS: u8 = 1;
const FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
const FREQ_SEARCH_STEP_HZ: u16 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: u8 = 32; // 32 PRN codes to search
const LONG_SAMPLES_LENGTH: u8 = 11; // ms


pub enum ChannelState {
    Idle,
    Acquiring,
    Tracking(u8),
    Lost,
}

pub enum SearchMode{
    ColdStart,  // No prior information, search all PRNs
    WarmStart,  // Use last known active PRNs to prioritize search
    SteadyState,  // After a fix, search rarely for new satellites
}

pub struct AcqController {
    mode: SearchMode,
}

impl AcqController {
    pub fn new() -> Self {
        Self {
            mode: SearchMode::ColdStart,
        }
    }

    pub fn update_mode(&mut self, trked_acount: usize) {
        self.mode = match trked_acount {
            0 => SearchMode::ColdStart,
            1..=4 => SearchMode::WarmStart,
            _ => SearchMode::SteadyState,
        };
    }

    pub fn get_pacing_and_list(&self, active_prns: &HashSet<u8>) -> (u64, Vec<u8>) {
        let (interval, search_size) = match self.mode {
            SearchMode::ColdStart => (500, PRN_SEARCH_ACQUISITION_TOTAL),
            SearchMode::WarmStart => (1000, 8), 
            SearchMode::SteadyState => (2000, 5), 
        };

        let mut candidates = (1..=PRN_SEARCH_ACQUISITION_TOTAL)
            .filter(|prn| !active_prns.contains(prn))
            .collect::<Vec<u8>>();

        candidates.truncate(search_size as usize);

        (interval, candidates)
    } 
}

#[derive(Debug, Clone)]
struct AcqError;

impl fmt::Display for AcqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error happens while doing signal acquisition!")
    }
}

impl Error for AcqError {}

impl<T> From<PoisonError<T>> for AcqError {
    fn from(_: PoisonError<T>) -> Self {
        AcqError
    }
}

#[derive(Debug, Clone)]
pub struct AcquisitionResult {
    pub prn: u8,
    pub code_phase: usize,
    pub carrier_freq: f32,
    pub mag_relative: f32,
    pub sample_global_index: usize,
}

impl AcquisitionResult {
    pub fn new(prn: u8) -> Self {
        Self {
            prn,
            code_phase: 0,
            carrier_freq: 0.0,
            mag_relative: 0.0,
            sample_global_index: 0,
        }
    }
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
    
    pub fn get_search_list(&self) -> Result<Vec<u8>, AcqError> {
        let active = self.active_prns.lock()?;
        Ok((1..=PRN_SEARCH_ACQUISITION_TOTAL as u8)
            .filter(|prn| !active.contains(prn))
            .collect())
    }
}

pub struct AcquisitionWorker {
    prn: u8,
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    fft_size: usize,
    freq_sampling_hz: f32,
    // doppler_table: &DopplerShiftTable,
    ca_code_samples_fft: Vec<Complex<f32>>,
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

        let ca_code_samples = generate_ca_code_samples(prn, freq_sampling_hz);
        let mut ca_code_samples_fft = vec![Complex::new(0.0, 0.0); ca_code_samples.len()];
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
            ca_code_samples_fft: ca_code_samples_fft,
            result_buf: vec![Complex::new(0.0, 0.0); fft_size],
            freq_replica: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    fn search_satellite(
        &mut self,
        samples_chunk: &[Complex<f32>],
        doppler_table: &[DopplerShiftTable],
        local_tail: usize,
    ) -> Option<AcquisitionResult> {
        let mut max_val: f32 = 0.0;
        let mut best_doper_freq: f32 = 0.0;
        let mut best_code_phase: usize = 0;
        let mut power_results = vec![0.0; self.fft_size];

        for doppler in ((-FREQ_SEARCH_ACQUISITION_HZ / 2.0) as usize..=(FREQ_SEARCH_ACQUISITION_HZ / 2.0) as usize)
            .step_by(FREQ_SEARCH_STEP_HZ as usize)
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
                    best_doper_freq = doppler as f32;
                    best_code_phase = idx;
                }
            }

            if self.is_good_satellite(&power_results, max_val) {
                return Some(AcquisitionResult {
                    prn: self.prn,
                    code_phase: best_code_phase,
                    carrier_freq: best_doper_freq,
                    mag_relative: max_val,
                    sample_global_index: local_tail + best_code_phase,
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
    freq_sampling_hz: f32,
    trk_manager: Arc<Mutex<TrackingManager>>
) -> Result<(), AcqError> {
    let manager = AcquisitionManager::new(500);
    let capacity = (FREQ_SEARCH_ACQUISITION_HZ as u16 / FREQ_SEARCH_STEP_HZ) as usize + 1;
    let fft_size = (freq_sampling_hz / (GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
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

    let mut acq_controller = AcqController::new();

    let mut workers = (1..=PRN_SEARCH_ACQUISITION_TOTAL)
        .into_par_iter()
        .filter_map(|prn| Some(AcquisitionWorker::new(prn, fft_size, freq_sampling_hz)))
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
        if head >= fft_size {
            let search_list = manager.get_search_list()?;

            if search_list.is_empty() {
                // All satellites are in tracking state, sleep long: 1s
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }

            local_tail = head - fft_size;
            multi_buffer.copy_to_slice(local_tail, &mut chunk_samples);

            let results: Vec<AcquisitionResult> = workers
                .par_iter_mut()
                .filter_map(|worker| worker.search_satellite(&chunk_samples, &doppler_table, local_tail))
                .collect();

            for result in results {
                manager.active_prns.lock()?.insert(result.prn);
                tracking_worker.spawn_tracking_thread(result);
            }

            manager.last_search_time = std::time::Instant::now();

        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
