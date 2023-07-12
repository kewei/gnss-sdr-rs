use std::collections::HashMap;

use crate::acquisition::{do_acquisition, AcquistionStatistics};
use crate::decoding::nav_decoding;
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
