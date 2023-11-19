use crate::gps_constants;
use crate::tracking::TrackingResult;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

pub struct Pos {
    x: f32,
    y: f32,
    z: f32,
    t: f32,
}

pub struct SubframeSyncStatus {
    buff_syn: VecDeque<i8>,
    sf_start_ind: u64,
    preamble_ind: u64,
}

impl SubframeSyncStatus {
    pub fn new() -> Self {
        Self {
            buff_syn: VecDeque::with_capacity(
                gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize,
            ),
            sf_start_ind: 0,
            preamble_ind: 0,
        }
    }
}

pub fn nav_decoding(
    tracking_result: Arc<Mutex<TrackingResult>>,
    cnt: u64,
    sf_sync_stat: &mut SubframeSyncStatus,
) -> Result<Pos, Box<dyn Error>> {
    if cnt > (1.0 / gps_constants::GPS_L1_CA_CODE_PERIOD_S) as u64 {
        let trk_result_clone = Arc::clone(&tracking_result);
        let trk_result = trk_result_clone
            .lock()
            .expect("Locking error in tracking_result in nav_decoding");
        sf_sync_stat
            .buff_syn
            .push_back(trk_result.i_prompt.signum() as i8);

        if sf_sync_stat.buff_syn.len() == gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize {
            if preamble_syn(&sf_sync_stat.buff_syn) {
                sf_sync_stat.preamble_ind = cnt;
            }
        }
    }
    todo!();
}

fn preamble_syn(preamble_buf: &VecDeque<i8>) -> bool {
    (0..gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize).fold(0, |accu, x| {
        accu + (preamble_buf[x] * gps_constants::GPS_CA_PREAMBLE[x % 8]) as i16
    }) == gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS
}
