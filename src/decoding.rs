use crate::tracking::TrackingResult;
use crate::{gps_constants, tracking};
use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{self, write};
use std::sync::{Arc, Mutex};

const BIT_SYNC_THRESHOLD: usize = 30;

#[derive(Clone, Debug)]
struct DecodingError(String);

impl fmt::Display for DecodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for DecodingError {}

#[derive(Debug, Clone)]
pub struct Pos {
    x: f32,
    y: f32,
    z: f32,
    t: f32,
}

impl Pos {
    pub fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            t: 0.0,
        }
    }
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
    sf_buffer_loc: usize,
    sf_cnt: usize,
    sf_start_biti: usize,
    tow_expected_ind: usize,
    buffer_loc_biti: Vec<usize>,
    buff_tow: Vec<i8>,
    tow_bits: String,
}

#[derive(Clone, Debug)]
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
            sf_buffer_loc: 0,
            sf_cnt: 0,
            sf_start_biti: 0,
            tow_expected_ind: 0,
            buffer_loc_biti: Vec::new(),
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
    buff_loc: usize,
    cnt: usize,
    navigation_sync_state: Arc<Mutex<NavSyncStatus>>,
) -> Result<SubframeMessage, Box<dyn Error>> {
    let trk_result = tracking_result
        .lock()
        .expect("Locking error in tracking_result in nav_decoding");
    let mut nav_sync_stat = navigation_sync_state
        .lock()
        .expect("Error in locking navigation_sync_status");
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
            buff_loc,
        );
    }

    if nav_sync_stat.sync_sw {
        if !nav_sync_stat.flag_frame_sync
            && nav_sync_stat.buff_preamble.len()
                == gps_constants::GPS_CA_PREAMBLE_LENGTH_BITS as usize
        {
            nav_sync_stat.flag_frame_sync = check_preamble_syn(&mut nav_sync_stat);
        }
        if nav_sync_stat.flag_frame_sync {
            nav_sync_stat.sf_buffer_loc = buff_loc;
            nav_sync_stat.sf_cnt = cnt;
            nav_sync_stat.sf_start_biti = nav_sync_stat.frame_bits.len()
                - gps_constants::GPS_CA_PREAMBLE_LENGTH_BITS as usize;
            nav_sync_stat.tow_expected_ind = cnt
                + (gps_constants::GPS_WORD_BITS * gps_constants::GPS_CA_TELEMETRY_SYMBOLS_PER_BIT)
                    as usize;
        }
    }

    if nav_sync_stat.flag_frame_sync && nav_sync_stat.sync_sw {
        if nav_sync_stat.frame_bits.len() % gps_constants::GPS_SUBFRAME_BITS as usize == 0 {
            println!("Bits: {:?}", nav_sync_stat.frame_bits);
            if let Some(sf_msg) = decode_subframe_message(&nav_sync_stat.frame_bits) {
                println!("{:?}", sf_msg);
                return Ok(sf_msg);
            } else {
                return Err(Box::new(DecodingError(
                    "Parity check fails in decoding subframe".into(),
                )));
            }
            nav_sync_stat.frame_bits.clear();
        }
    }
    Err(Box::new(DecodingError("Navigation decoding fails".into())))
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

fn bit_accumulation(
    nav_stats: &mut NavSyncStatus,
    cnt: usize,
    i_p: f32,
    loop_ms: usize,
    buff_loc: usize,
) {
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
        nav_stats.buffer_loc_biti.push(buff_loc);
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

fn decode_subframe_message(sf_bits: &[i8]) -> Option<SubframeMessage> {
    let word_length = gps_constants::GPS_WORD_BITS as usize;
    if !parity_check(&sf_bits[0..word_length]) {
        return None;
    };
    decode_tlm(&sf_bits[0..word_length]);
    if !parity_check(&sf_bits[word_length..2 * word_length]) {
        return None;
    };
    let sf_msg = decode_tow(&sf_bits[word_length..2 * word_length]);
    Some(sf_msg)
}

fn decode_tlm(bits: &[i8]) -> SubframeMessage {
    todo!()
}
fn decode_tow(bits: &[i8]) -> SubframeMessage {
    let mut tow_bits = "".to_string();
    for i in 0..gps_constants::GPS_TOW_BITS {
        for i in 0..gps_constants::GPS_WORD_BITS as usize {
            tow_bits += if bits[i] == 1 { "1" } else { "0" };
        }
    }
    let subframe_message = SubframeMessage {
        tow: u32::from_str_radix(tow_bits.as_str(), 2)
            .expect("Error happens when parsing TOW bits to u32"),
    };
    subframe_message
}

fn parity_check(bits: &[i8]) -> bool {
    let mut parity_bits = Vec::with_capacity(6);

    parity_bits[0] = bits[0]
        * bits[2]
        * bits[3]
        * bits[4]
        * bits[6]
        * bits[7]
        * bits[11]
        * bits[12]
        * bits[13]
        * bits[14]
        * bits[15]
        * bits[18]
        * bits[19]
        * bits[21]
        * bits[24];
    parity_bits[1] = bits[1]
        * bits[3]
        * bits[4]
        * bits[5]
        * bits[7]
        * bits[8]
        * bits[12]
        * bits[13]
        * bits[14]
        * bits[15]
        * bits[16]
        * bits[19]
        * bits[20]
        * bits[22]
        * bits[25];
    parity_bits[2] = bits[0]
        * bits[2]
        * bits[4]
        * bits[5]
        * bits[6]
        * bits[8]
        * bits[9]
        * bits[13]
        * bits[14]
        * bits[15]
        * bits[16]
        * bits[17]
        * bits[20]
        * bits[21]
        * bits[23];
    parity_bits[3] = bits[1]
        * bits[3]
        * bits[5]
        * bits[6]
        * bits[7]
        * bits[9]
        * bits[10]
        * bits[14]
        * bits[15]
        * bits[16]
        * bits[17]
        * bits[18]
        * bits[21]
        * bits[22]
        * bits[24];
    parity_bits[4] = bits[1]
        * bits[2]
        * bits[4]
        * bits[6]
        * bits[7]
        * bits[8]
        * bits[10]
        * bits[11]
        * bits[15]
        * bits[16]
        * bits[17]
        * bits[18]
        * bits[19]
        * bits[22]
        * bits[23]
        * bits[25];
    parity_bits[5] = bits[0]
        * bits[4]
        * bits[6]
        * bits[7]
        * bits[9]
        * bits[10]
        * bits[11]
        * bits[12]
        * bits[14]
        * bits[16]
        * bits[20]
        * bits[23]
        * bits[24]
        * bits[25];

    let result: i8 = (0..6).map(|i| parity_bits[i] - bits[26 + i]).sum();

    result == 0
}
