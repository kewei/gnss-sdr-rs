use std::collections::HashMap;

use crate::acquisition::{do_acquisition, AcquistionStatistics};
use crate::decoding::nav_decoding;
use crate::gps_constants;
use crate::tracking::{do_track, TrackingStatistics};

#[derive(Debug, Clone, Copy)]
pub enum ProcessStage {
    SignalAcquistion,
    SignalTracking,
    MesageDecoding,
}

pub fn do_data_process(
    data_in: &Vec<i16>,
    freq_sampling: f32,
    freq_IF: f32,
    stage: &mut ProcessStage,
    acquisition_statistic: &mut Vec<AcquistionStatistics>,
    tracking_statistic: &mut HashMap<i16, TrackingStatistics>,
) {
    //let mut next_stage = stage;
    //let mut acq_statistic = acquire_statistic;
    //let mut tr_statistic = track_statistic;
    match stage {
        ProcessStage::SignalAcquistion => {
            if let Ok(()) = do_acquisition(data_in, acquisition_statistic, freq_sampling, freq_IF) {
                for acq_result in acquisition_statistic {
                    let mut tracking_result = &mut TrackingStatistics::new();
                    if let Some(result) = tracking_statistic.get_mut(&acq_result.prn) {
                        tracking_result = result;
                    };
                    tracking_result
                        .tracking_stat
                        .carrier_freq
                        .push(acq_result.doppler_freq + freq_IF);
                    tracking_result
                        .tracking_stat
                        .code_freq
                        .push(gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S);
                    tracking_result.tracking_stat.code_phase_error.push(0.0);
                    tracking_result.tracking_stat.carrier_phase_error.push(0.0);
                    tracking_result.tracking_stat.code_error_filtered.push(0.0);
                    tracking_result.tracking_stat.code_error.push(0.0);
                    tracking_result
                        .tracking_stat
                        .carrier_error_filtered
                        .push(0.0);
                    tracking_result.tracking_stat.carrier_error.push(0.0);

                    let mut ca_code = acq_result.ca_code.clone();
                    ca_code.insert(
                        0,
                        ca_code[gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize - 1],
                    );
                    ca_code.push(ca_code[0]);

                    let code_phase_step: f32 =
                        gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / freq_sampling;
                    let num_ca_code_samples = (gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS
                        / code_phase_step)
                        .ceil() as usize;
                    let ca_code_prompt: Vec<f32> = (0..num_ca_code_samples)
                        .map(|x| ca_code[(x as f32 * code_phase_step).ceil() as usize + 1] as f32)
                        .collect();
                    tracking_result.ca_code_prompt.push(ca_code_prompt);
                }
            } else {
            };
        }
        ProcessStage::SignalTracking => {
            if let Ok(()) = do_track(
                data_in,
                acquisition_statistic,
                tracking_statistic,
                freq_sampling,
                freq_IF,
            ) {
            } else {
                todo!(); // do tracking again with new data
            };
        }
        ProcessStage::MesageDecoding => {
            if let Ok(pos_result) = nav_decoding(tracking_statistic) {
            } else {
                todo!(); // do tracking again with new data
            }
        }
    }
}
