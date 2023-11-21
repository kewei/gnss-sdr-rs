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
    flag_syn: bool,
    sf_start_ind: u64,
    preamble_ind: u64,
    tow_expected_ind: u64,
    tow_syn: Vec<i8>,
    tow_bits: String,
}

pub struct SubframeMessage {
    tow: u32,
}

impl SubframeSyncStatus {
    pub fn new() -> Self {
        Self {
            buff_syn: VecDeque::with_capacity(
                gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize,
            ),
            flag_syn: false,
            sf_start_ind: 0,
            preamble_ind: 0,
            tow_expected_ind: 0,
            tow_syn: Vec::with_capacity(
                (gps_constants::GPS_WORD_BITS * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                    as usize,
            ),
            tow_bits: "".to_owned(),
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
        let i_p = trk_result.i_prompt.signum() as i8;
        sf_sync_stat.buff_syn.push_back(i_p);

        if sf_sync_stat.buff_syn.len() == gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize {
            if !sf_sync_stat.flag_syn {
                if preamble_syn(&sf_sync_stat.buff_syn) {
                    sf_sync_stat.preamble_ind = cnt;
                    sf_sync_stat.flag_syn = true;
                    sf_sync_stat.tow_expected_ind = cnt
                        + (gps_constants::GPS_WORD_BITS
                            * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                            as u64;
                }
                tlm_parity_check();
            } else if cnt >= sf_sync_stat.tow_expected_ind
                && cnt
                    < sf_sync_stat.tow_expected_ind
                        + (gps_constants::GPS_TOW_BITS
                            * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                            as u64
            {
                sf_sync_stat.tow_syn.push(if i_p == 1 { 1 } else { 0 });
            } else if cnt
                == sf_sync_stat.tow_expected_ind
                    + (gps_constants::GPS_TOW_BITS
                        * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                        as u64
            {
                for i in 0..gps_constants::GPS_TOW_BITS {
                    if sf_sync_stat.tow_syn[(i * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                        as usize
                        ..((i + 1) * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT) as usize]
                        .iter()
                        .sum::<i8>()
                        == gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT as i8
                    {
                        sf_sync_stat.tow_bits = sf_sync_stat.tow_bits.to_owned() + "1";
                    } else if sf_sync_stat.tow_syn[(i
                        * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                        as usize
                        ..((i + 1) * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT) as usize]
                        .iter()
                        .sum::<i8>()
                        == 0
                    {
                        sf_sync_stat.tow_bits = sf_sync_stat.tow_bits.to_owned() + "0";
                    } else {
                        sf_sync_stat.flag_syn = false;
                        sf_sync_stat.buff_syn.clear();
                    }
                }
                let subframe_message = SubframeMessage {
                    tow: u32::from_str_radix(sf_sync_stat.tow_bits.as_str(), 2)
                        .expect("Error happens when parsing TOW bits to u32"),
                };
            }
            how_parity_check();
        }
    }
    todo!();
}

fn preamble_syn(preamble_buf: &VecDeque<i8>) -> bool {
    ((0..gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize).fold(0, |accu, x| {
        accu + (preamble_buf[x] * gps_constants::GPS_CA_PREAMBLE[x % 8]) as i16
    }))
    .abs()
        == gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS
}

fn tlm_parity_check() {
    todo!()
}

fn how_parity_check() {
    todo!()
}
