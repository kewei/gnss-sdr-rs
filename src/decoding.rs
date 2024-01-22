use crate::gps_constants;
use crate::tracking::TrackingResult;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

const BIT_SYNC_THRESHOLD: usize = 30; //30 bits

pub struct Pos {
    x: f32,
    y: f32,
    z: f32,
    t: f32,
}

pub struct NavSyncStatus {
    flag_bit_sync: bool,
    biti: usize,
    bit_sync_buff: Vec<usize>,
    flag_frame_sync: bool,
    frame_sync_ind: usize,
    sf_start_ind: usize,
    preamble_ind: usize,
    buff_preamble: VecDeque<i8>,
    flag_tow_sync: bool,
    tow_expected_ind: usize,
    buff_tow: Vec<i8>,
    tow_bits: String,
}

pub struct SubframeMessage {
    tow: u32,
}

impl NavSyncStatus {
    pub fn new() -> Self {
        Self {
            flag_bit_sync: false,
            biti: 0,
            bit_sync_buff: vec![0; gps_constants::GPS_L1_CA_BIT_PERIOD_MS as usize],
            flag_frame_sync: false,
            frame_sync_ind: 0,
            sf_start_ind: 0,
            preamble_ind: 0,
            buff_preamble: VecDeque::with_capacity(
                gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize,
            ),
            flag_tow_sync: false,
            tow_expected_ind: 0,
            buff_tow: Vec::with_capacity(
                (gps_constants::GPS_WORD_BITS * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                    as usize,
            ),
            tow_bits: "".to_owned(),
        }
    }
}

pub fn nav_decoding(
    tracking_result: Arc<Mutex<TrackingResult>>,
    cnt: usize,
    navigation_sync_state: Arc<Mutex<NavSyncStatus>>,
) -> Result<Pos, Box<dyn Error>> {
    let trk_result = tracking_result
        .lock()
        .expect("Locking error in tracking_result in nav_decoding");
    let mut nav_sync_stat = navigation_sync_state
        .lock()
        .expect("Error in locking navigation_sync_status");
    let i_p = trk_result.i_prompt.signum() as i8;
    nav_sync_stat.buff_preamble.push_back(i_p);
    nav_sync_stat.biti = cnt % gps_constants::GPS_L1_CA_BIT_PERIOD_MS as usize;
    if !nav_sync_stat.flag_bit_sync && cnt > (1.0 / gps_constants::GPS_L1_CA_CODE_PERIOD_S) as usize
    {
        nav_sync_stat.flag_bit_sync = check_bit_sync(&mut nav_sync_stat, &trk_result);
    }
    if nav_sync_stat.flag_bit_sync {
        nav_sync_stat.preamble_ind = cnt;
        nav_sync_stat.tow_expected_ind = cnt
            + (gps_constants::GPS_WORD_BITS * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                as usize;
        nav_sync_stat.flag_frame_sync = check_preamble_syn(&nav_sync_stat);
        tlm_parity_check();
        if cnt >= nav_sync_stat.tow_expected_ind {
            nav_sync_stat.buff_tow.push(if i_p == 1 { 1 } else { 0 });
        }
        if cnt + 1
            == nav_sync_stat.tow_expected_ind
                + (gps_constants::GPS_TOW_BITS * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                    as usize
        {
            for i in 0..gps_constants::GPS_TOW_BITS {
                let sum_v = nav_sync_stat.buff_tow[(i
                    * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                    as usize
                    ..((i + 1) * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT) as usize]
                    .iter()
                    .sum::<i8>();
                if sum_v == gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT as i8 {
                    nav_sync_stat.tow_bits = nav_sync_stat.tow_bits.to_owned() + "1";
                } else if sum_v == 0 {
                    nav_sync_stat.tow_bits = nav_sync_stat.tow_bits.to_owned() + "0";
                } else {
                    nav_sync_stat.flag_bit_sync = false;
                    nav_sync_stat.buff_preamble.clear();
                }
            }
            let subframe_message = SubframeMessage {
                tow: u32::from_str_radix(nav_sync_stat.tow_bits.as_str(), 2)
                    .expect("Error happens when parsing TOW bits to u32"),
            };
        }
        how_parity_check();
    }
    todo!();
}

fn check_bit_sync(nav_stats: &mut NavSyncStatus, trk_result: &TrackingResult) -> bool {
    if trk_result.old_i_prompt * trk_result.i_prompt < 0.0 {
        nav_stats.bit_sync_buff[nav_stats.biti] += 1;
        let (i_max, &v_max) = nav_stats
            .bit_sync_buff
            .iter()
            .enumerate()
            .max_by(|(_, &a), (_, &b)| a.cmp(&b))
            .unwrap();
        nav_stats.frame_sync_ind = i_max;

        if v_max == BIT_SYNC_THRESHOLD {
            return true;
        }
    }
    false
}

fn check_preamble_syn(nav_sync_stat: &NavSyncStatus) -> bool {
    ((0..gps_constants::GPS_CA_PREAMBLE_LENGTH_SYMBOLS as usize).fold(0, |accu, x| {
        accu + (nav_sync_stat.buff_preamble[x] * gps_constants::GPS_CA_PREAMBLE[x % 8]) as i16
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
