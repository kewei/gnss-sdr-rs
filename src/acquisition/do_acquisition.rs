use crate::acquisition::doppler_shift::{DopplerShiftTable, apply_doppler_shift};
use crate::constants::gps_property_constants::{
    GPS_L1_CA_CODE_LENGTH_CHIPS, GPS_L1_CA_CODE_RATE_CHIPS_PER_S,
};
use crate::tracking::do_tracking::TrackingMessage;
use crate::utilities::ca_code::generate_ca_code_samples;
use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
use crate::utilities::help_fn::convert_i8_to_complex32;
use num::Complex;
use crossbeam_channel::{Sender, Receiver};
use rayon::prelude::*;
use rustfft::{Fft, FftPlanner};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::simd::f32x8;
use std::simd::num::SimdFloat;
use std::sync::{Arc, PoisonError};

// const FFT_LENGTH_MS: u8 = 1;
const FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
const FREQ_SEARCH_STEP_HZ: u16 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: u8 = 32; // 32 PRN codes to search
const LONG_SAMPLES_LENGTH: usize = 10; // ms

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

        let mask: u32 = candidates.iter().fold(0, |acc, x| acc | (1 << (x - 1)));

        (interval, mask)
    }
}

#[derive(Debug, Clone)]
pub struct AcqError;

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
    pub code_phase_samples: usize,
    pub code_phase_chips: f32,
    pub carrier_freq: f32,
    pub fs: f32,
    pub mag_relative: f32,
    pub sample_global_index: usize,
}

impl AcquisitionResult {
    pub fn new(prn: u8) -> Self {
        Self {
            prn,
            code_phase_samples: 0,
            code_phase_chips: 0.0,
            carrier_freq: 0.0,
            fs: 0.0,
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

        let ca_code_samples =
            generate_ca_code_samples(prn, GPS_L1_CA_CODE_RATE_CHIPS_PER_S, freq_sampling_hz);
        let mut ca_code_samples_fft = convert_i8_to_complex32(ca_code_samples);
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
        }
    }

    fn search_satellite(
        &mut self,
        samples_chunk: &[Complex<f32>],
        doppler_table: &[DopplerShiftTable],
        local_tail: usize,
        num_integrations: usize,
    ) -> Option<AcquisitionResult> {
        let mut global_max_val: f32 = 0.0;
        let mut best_doppler_freq: f32 = 0.0;
        let mut best_code_phase: usize = 0;
        let mut best_power_results = vec![0.0; self.fft_size];
        let mut accumulated_power = vec![0.0; self.fft_size];

        for doppler in doppler_table.iter() {
            accumulated_power.fill(0.0);

            for c in 0..num_integrations {
                let offset = c * self.fft_size;
                let chunk = &samples_chunk[offset..offset + self.fft_size];
                apply_doppler_shift(
                    chunk,
                    doppler,
                    &mut self.result_buf,
                );
                self.fft.process(&mut self.result_buf);

                for i in 0..self.fft_size {
                    self.result_buf[i] *= self.ca_code_samples_fft[i].conj();
                }

                self.ifft.process(&mut self.result_buf);

                for (idx, val) in self.result_buf.iter().enumerate() {
                    accumulated_power[idx] += val.norm_sqr();
                }
            }

            let mut local_max = 0.0;
            let mut local_best_phase = 0;
            for (idx, &power) in accumulated_power.iter().enumerate() {
                if power > local_max {
                    local_max = power;
                    local_best_phase = idx;
                }
            }

            if local_max > global_max_val {
                global_max_val = local_max;
                best_doppler_freq = doppler.doppler_freq_hz;
                best_code_phase = local_best_phase;
                best_power_results.copy_from_slice(&accumulated_power);
            }

            if self.is_good_satellite(&best_power_results, global_max_val) {
                return Some(AcquisitionResult {
                    prn: self.prn,
                    code_phase_samples: best_code_phase,
                    code_phase_chips: best_code_phase as f32 * GPS_L1_CA_CODE_RATE_CHIPS_PER_S
                        / self.freq_sampling_hz,
                    carrier_freq: best_doppler_freq,
                    fs: self.freq_sampling_hz,
                    mag_relative: global_max_val,
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
    f_if: f32,
    to_tracking: Sender<AcquisitionResult>,
    from_tracking: Receiver<TrackingMessage>,
) -> Result<(), AcqError> {
    let num_integrations = 10;
    let capacity = (FREQ_SEARCH_ACQUISITION_HZ as u16 / FREQ_SEARCH_STEP_HZ) as usize + 1;
    let fft_size = (freq_sampling_hz
        / (GPS_L1_CA_CODE_RATE_CHIPS_PER_S / GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let mut doppler_table = Vec::with_capacity(capacity);
    for i in 0..capacity {
        let doppler_freq =
            -FREQ_SEARCH_ACQUISITION_HZ / 2.0 + i as f32 * FREQ_SEARCH_STEP_HZ as f32;
        doppler_table.push(DopplerShiftTable::new(
            f_if,
            doppler_freq,
            freq_sampling_hz,
            fft_size,
        ));
    }

    let mut active_prns = HashSet::new();

    let mut acq_manager = AcquisitionManager::new();

    let mut workers = (1..=PRN_SEARCH_ACQUISITION_TOTAL)
        .into_par_iter()
        .filter_map(|prn| Some(AcquisitionWorker::new(prn, fft_size, freq_sampling_hz)))
        .collect::<Vec<AcquisitionWorker>>();

    let samples_integration_size = fft_size * LONG_SAMPLES_LENGTH;
    let mut chunk_samples = vec![Complex::new(0.0, 0.0); samples_integration_size];
    let mut last_run = std::time::Instant::now();

    loop {
        while let Ok(msg) = from_tracking.try_recv() {
            match msg {
                TrackingMessage::SatelliteLost(prn) => {
                    active_prns.remove(&prn);
                }
                TrackingMessage::SatelliteLocked(prn) => {
                    active_prns.insert(prn);
                }
            }
        }

        acq_manager.update_mode(active_prns.len());
        let (interval_ms, mask) = acq_manager.get_pacing_and_list(&active_prns);

        if last_run.elapsed().as_millis() < interval_ms as u128 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let head = multi_buffer.get_head();

        if head >= samples_integration_size {
            let local_tail = head - samples_integration_size;
            multi_buffer.copy_to_slice(local_tail, &mut chunk_samples);
            let results: Vec<AcquisitionResult> = workers
                .par_iter_mut()
                .enumerate()
                .filter_map(|(i, worker)| {
                    let prn = i as u8 + 1;
                    if (mask >> (prn - 1)) & 1 == 1 {
                        worker.search_satellite(&chunk_samples, &doppler_table, local_tail, LONG_SAMPLES_LENGTH)
                    } else {
                        None
                    }
                })
                .collect();

            for result in results {
                let prn = result.prn;
                if to_tracking.send(result).is_ok() {
                    active_prns.insert(prn);
                }
            }

            last_run = std::time::Instant::now();
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex32;
    use std::collections::HashSet;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    #[test]
    fn test_acquisition_manager_initialization() {
        let manager = AcquisitionManager::new();
        // Verifies the manager starts in ColdStart as expected
        assert!(matches!(manager.mode, SearchMode::ColdStart));
    }

    #[test]
    fn test_update_mode_transitions() {
        let mut manager = AcquisitionManager::new();

        // 1 to 4 satellites should trigger WarmStart
        manager.update_mode(3);
        assert!(matches!(manager.mode, SearchMode::WarmStart));

        // 5 or more satellites should trigger SteadyState
        manager.update_mode(5);
        assert!(matches!(manager.mode, SearchMode::SteadyState));

        // Losing all satellites should revert to ColdStart
        manager.update_mode(0);
        assert!(matches!(manager.mode, SearchMode::ColdStart));
    }

    #[test]
    fn test_get_pacing_and_list_cold_start() {
        let manager = AcquisitionManager::new();
        let active_prns = HashSet::new();

        let (interval, mask) = manager.get_pacing_and_list(&active_prns);

        // Cold start interval is 500ms
        assert_eq!(interval, 500);

        // All 32 bits should be set for PRN 1-32 (0xFFFFFFFF)
        assert_eq!(mask, 0xFFFFFFFF);
    }

    #[test]
    fn test_get_pacing_and_list_filtering_warm_start() {
        let mut manager = AcquisitionManager::new();
        manager.update_mode(3); // Sets to WarmStart (interval: 1000, size: 8)

        let mut active_prns = HashSet::new();
        active_prns.insert(1); // Exclude PRN 1
        active_prns.insert(2); // Exclude PRN 2
        active_prns.insert(3); // Exclude PRN 3

        let (interval, mask) = manager.get_pacing_and_list(&active_prns);

        assert_eq!(interval, 1000);

        // Candidates should be 4, 5, 6, 7, 8, 9, 10, 11 (8 items)
        // Bitmask calculation: 1<<3 + 1<<4 + 1<<5 + 1<<6 + 1<<7 + 1<<8 + 1<<9 + 1<<10
        // 8 + 16 + 32 + 64 + 128 + 256 + 512 + 1024 = 2040
        assert_eq!(mask, 2040);
    }

    #[test]
    fn test_acquisition_with_real_data() {
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

        let mut raw_bytes = vec![0u8; MS_SAMPLES];
        file.read_exact(&mut raw_bytes)
            .expect("Failed to read 1ms of samples");
        let mut raw_samples = vec![Complex32::new(0.0, 0.0); MS_SAMPLES];
        raw_samples = raw_bytes.iter().map(|x| Complex32::new((*x as i8) as f32, 0.0)).collect();

        let doppler_start = -7000.0;
        let doppler_end = 7000.0;
        let step = 500.0;

        let mut doppler_tables = Vec::new();
        let mut current_doppler = doppler_start;

        while current_doppler <= doppler_end {
            doppler_tables.push(DopplerShiftTable::new(IF, current_doppler, FS, MS_SAMPLES/NUM_INTEGRATIONS));
            current_doppler += step;
        }

        let true_satellites = vec![1, 2, 3, 6, 9, 11, 14, 18, 19, 22, 28, 32];
        let mut test_prn = 1;
        while test_prn < 33 {
            let mut worker = AcquisitionWorker::new(test_prn, MS_SAMPLES/NUM_INTEGRATIONS, FS);

            let result = worker.search_satellite(&raw_samples, &doppler_tables, 0, NUM_INTEGRATIONS);

            match result {
                Some(acq) => {
                    println!("SUCCESS: Acquired PRN {}!", acq.prn);
                    println!("  Doppler Shift:  {} Hz", acq.carrier_freq);
                    println!("  Code Phase:     {} samples", acq.code_phase_samples);
                    println!("  Code Phase:     {} chips", acq.code_phase_chips);
                    println!("  Relative Power: {}", acq.mag_relative);

                    assert!(&true_satellites.contains(&test_prn), "Acquired PRN {} which is not in the true satellite list!", test_prn);
                }
                None => {
                    println!(
                        "PRN {} not found in this {}ms chunk (satellite likely not visible).",
                        test_prn,
                        NUM_INTEGRATIONS
                    );
                }
            }
            test_prn += 1;
        }
    }
}
