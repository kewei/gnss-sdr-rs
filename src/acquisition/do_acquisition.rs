use crate::acquisition::doppler_shift::{DopplerShiftTable, apply_doppler_shift};
use crate::constants::gps_def_constants::{
    GPS_L1_CA_CODE_LENGTH_CHIPS, GPS_L1_CA_CODE_RATE_CHIPS_PER_S,
};
use crate::tracking::do_tracking::TrackingManager;
use crate::utilities::ca_code::generate_ca_code_samples;
use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
use num::Complex;
use rayon::prelude::*;
use rustfft::{Fft, FftPlanner};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::simd::f32x8;
use std::simd::num::SimdFloat;
use std::sync::{Arc, PoisonError, RwLock};

const FFT_LENGTH_MS: u8 = 1;
const FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
const FREQ_SEARCH_STEP_HZ: u16 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: u8 = 32; // 32 PRN codes to search
const LONG_SAMPLES_LENGTH: u8 = 11; // ms

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelState {
    Idle,
    Acquiring,
    Tracking(u8),
    Lost,
}

pub enum SearchMode {
    ColdStart,   // No prior information, search all PRNs
    WarmStart,   // Use last known active PRNs to prioritize search
    SteadyState, // After a fix, search rarely for new satellites
}

pub struct AcquisitionManager {
    mode: SearchMode,
}

impl AcquisitionManager {
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

    pub fn get_pacing_and_list(&self, active_prns: &HashSet<u8>) -> (u64, u32) {
        let (interval, search_size) = match self.mode {
            SearchMode::ColdStart => (500, PRN_SEARCH_ACQUISITION_TOTAL),
            SearchMode::WarmStart => (1000, 8),
            SearchMode::SteadyState => (2000, 5),
        };

        let mut candidates = (1..=PRN_SEARCH_ACQUISITION_TOTAL)
            .filter(|prn| !active_prns.contains(prn))
            .collect::<Vec<u8>>();
        candidates.truncate(search_size as usize);

        let mask: u32 = candidates.iter().fold(0,|acc, x| acc | (1 << (x - 1)));

        (interval, mask)
    }
}

#[derive(Debug, Clone)]
struct AcqError;

impl fmt::Display for AcqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Acquisition Error!")
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

        for doppler in ((-FREQ_SEARCH_ACQUISITION_HZ / 2.0) as usize
            ..=(FREQ_SEARCH_ACQUISITION_HZ / 2.0) as usize)
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

pub fn run(
    multi_buffer: Arc<MulticastRingBuffer>,
    freq_sampling_hz: f32,
    trk_manager: Arc<RwLock<TrackingManager>>,
) -> Result<(), AcqError> {
    let capacity = (FREQ_SEARCH_ACQUISITION_HZ as u16 / FREQ_SEARCH_STEP_HZ) as usize + 1;
    let fft_size = (freq_sampling_hz
        / (GPS_L1_CA_CODE_RATE_CHIPS_PER_S / GPS_L1_CA_CODE_LENGTH_CHIPS))
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

    let mut acq_manager = AcquisitionManager::new();

    let mut workers = (1..=PRN_SEARCH_ACQUISITION_TOTAL)
        .into_par_iter()
        .filter_map(|prn| Some(AcquisitionWorker::new(prn, fft_size, freq_sampling_hz)))
        .collect::<Vec<AcquisitionWorker>>();

    let mut local_tail = 0;
    let mut chunk_samples = vec![Complex::new(0.0, 0.0); fft_size];
    let mut last_run = std::time::Instant::now();

    loop {
        let active_set_arc = trk_manager.read()?.active_prns.clone();
        let active_set = active_set_arc.read()?.clone();

        acq_manager.update_mode(active_set.len());
        let (interval_ms, mask) = acq_manager.get_pacing_and_list(&active_set);

        if last_run.elapsed().as_millis() < interval_ms as u128 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let head = multi_buffer.get_head();
        if head >= fft_size {

            local_tail = head - fft_size;
            multi_buffer.copy_to_slice(local_tail, &mut chunk_samples);
            let results: Vec<AcquisitionResult> = workers
                .par_iter_mut()
                .enumerate()
                .filter_map(|(i, worker)| {
                    let prn = i as u8 + 1;
                    if (mask >> (prn - 1)) & 1 == 1 {
                        worker.search_satellite(&chunk_samples, &doppler_table, local_tail)
                    } else {
                        None
                    }
                })
                .collect();

            let mut trk = trk_manager.write()?;
            for result in results {
                // acq_manager.active_prns.lock()?.insert(result.prn);
                trk.assign_tracking(result);
            }

            last_run = std::time::Instant::now();
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
