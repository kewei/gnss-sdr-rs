use crate::tracking::TrackingResult;
use crate::{gps_constants, tracking};
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

const BIT_SYNC_THRESHOLD: usize = 30;

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
    bit_code_cnt: usize,
    i_p: f32,
    loop_sw: bool,
    frame_bits: Vec<i8>,
    sync_sw: bool,
    preamble_ind: usize,
    buff_preamble: VecDeque<i8>,
    polarity: i8,
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
            bit_code_cnt: 0,
            i_p: 0.0,
            loop_sw: false,
            frame_bits: Vec::new(),
            sync_sw: false,
            preamble_ind: 0,
            buff_preamble: VecDeque::with_capacity(
                gps_constants::GPS_CA_PREAMBLE_LENGTH_BITS as usize,
            ),
            polarity: -1,
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
        bit_accumulation(
            &mut nav_sync_stat,
            cnt,
            trk_result.i_prompt,
            tracking::LOOP_MS,
        );
    }

    if !nav_sync_stat.flag_frame_sync
        && nav_sync_stat.buff_preamble.len() == gps_constants::GPS_CA_PREAMBLE_LENGTH_BITS as usize
    {
        nav_sync_stat.flag_frame_sync = check_preamble_syn(&mut nav_sync_stat);
    }
    nav_sync_stat.preamble_ind = cnt;
    nav_sync_stat.tow_expected_ind = cnt
        + (gps_constants::GPS_WORD_BITS * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT) as usize;
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
            let sum_v =
                nav_sync_stat.buff_tow[(i * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
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

fn bit_accumulation(nav_stats: &mut NavSyncStatus, cnt: usize, i_p: f32, loop_ms: usize) {
    nav_stats.sync_sw = false;
    if nav_stats.biti == nav_stats.frame_sync_ind {
        nav_stats.bit_code_cnt = 1;
        nav_stats.i_p = i_p;
    } else {
        nav_stats.i_p += i_p;
    }

    nav_stats.loop_sw = nav_stats.bit_code_cnt % loop_ms == 0;

    if nav_stats.biti
        == (nav_stats.frame_sync_ind + gps_constants::GPS_L1_CA_BIT_PERIOD_MS as usize - 1)
    {
        let bit = if nav_stats.i_p > 0.0 { 1_i8 } else { -1_i8 };
        nav_stats.frame_bits.push(bit);
        nav_stats.sync_sw = true;
        if !nav_stats.flag_frame_sync {
            nav_stats.buff_preamble.push_back(bit);
        }
    }
    nav_stats.bit_code_cnt += 1;
}

fn check_preamble_syn(nav_sync_stat: &mut NavSyncStatus) -> bool {
    let corr = (0..gps_constants::GPS_CA_PREAMBLE_LENGTH_BITS as usize).fold(0, |accu, x| {
        accu + (nav_sync_stat.buff_preamble[x] * gps_constants::GPS_CA_PREAMBLE[x % 8])
    });
    if corr.abs() == gps_constants::GPS_CA_PREAMBLE_LENGTH_BITS as i8 {
        nav_sync_stat.polarity = corr.signum();
        true
    } else {
        false
    }
}

fn tlm_parity_check() {
    todo!()
}

fn how_parity_check() {
    todo!()
}
