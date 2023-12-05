use chrono::{offset::TimeZone, DateTime, NaiveDateTime};
use chrono::{FixedOffset, Utc};
use core::fmt;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug)]
struct RinexError(String);

impl fmt::Display for RinexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rinex reading error: {}", self.0)
    }
}

impl Error for RinexError {}

#[derive(Debug)]
pub struct GnssRinexNavHeader {
    rinex_v: String,
    file_type: String,
    system: String,
    pgm: String,
    run_by: String,
    file_creation_t: DateTime<Utc>,
    comment_line: String,
    iono_corr: Vec<HashMap<String, String>>,
    time_sys_corr: HashMap<String, String>,
    leap_sec: u8,
}

impl GnssRinexNavHeader {
    fn new() -> Self {
        Self {
            rinex_v: "".to_string(),
            file_type: "".to_string(),
            system: "".to_string(),
            pgm: "".to_string(),
            run_by: "".to_string(),
            file_creation_t: Utc::now(),
            comment_line: "".to_string(),
            iono_corr: Vec::new(),
            time_sys_corr: HashMap::new(),
            leap_sec: 0,
        }
    }
}

pub struct BroadCastOrbit1 {
    iode: f32,
    crs: f32,
    delta_n: f32,
    m0: f32,
}
pub struct BroadCastOrbit2 {
    cuc: f32,
    e_eccentricity: f32,
    cus: f32,
    sqrt_a: f32,
}

pub struct BroadCastOrbit3 {
    toe: f32,
    cic: f32,
    omega0: f32,
    cis: f32,
}

pub struct BroadCastOrbit4 {
    i0: f32,
    crc: f32,
    omega: f32,
    omega_dot: f32,
}

pub struct BroadCastOrbit5 {
    idot: f32,
    code_on_l2: f32,
    gps_week: f32,
    l2_p_flag: f32,
}

pub struct BroadCastOrbit6 {
    sv_accuracy: f32,
    sv_health: f32,
    tgd: f32,
    iodc: f32,
}

pub struct BroadCastOrbit7 {
    t_transmission_message: f32,
    fit_interval_hours: f32,
}

pub struct GnssRinexNavRecord {
    satellite_sys: String,
    satellite_number: u16,
    time: NaiveDateTime,
    sv_clock_bias: f32,
    sv_clock_drift: f32,
    sv_clocl_drift_rate: f32,
    orbit1: BroadCastOrbit1,
    orbit2: BroadCastOrbit2,
    orbit3: BroadCastOrbit3,
    orbit4: BroadCastOrbit4,
    orbit5: BroadCastOrbit5,
    orbit6: BroadCastOrbit6,
    orbit7: BroadCastOrbit7,
}
pub struct GpsRinexNavData {
    rinex_header: GnssRinexNavHeader,
    rinex_data_record: GnssRinexNavRecord,
}

pub fn get_sats_from_rinex(
    file_name: &str,
) -> Result<HashMap<String, HashMap<String, String>>, Box<dyn Error>> {
    let mut rinex_header = GnssRinexNavHeader::new();
    let file = File::open(file_name)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut line_len = reader.read_line(&mut line)?;
    if line_len == 0 {
        return Err(Box::new(RinexError(
            "The GNSS RINEX navigation data is expected".into(),
        )));
    }
    rinex_header.rinex_v = line[0..14].trim().to_string();
    rinex_header.file_type = line[14..38].trim().to_string();
    rinex_header.system = line[38..55].trim().to_string();
    line.clear();

    line_len = reader.read_line(&mut line)?;
    if line_len == 0 {
        return Err(Box::new(RinexError(
            "The GNSS RINEX navigation data is expected".into(),
        )));
    }
    rinex_header.pgm = line[0..20].trim().to_string();
    rinex_header.run_by = line[20..40].trim().to_string();
    let mut file_creation_t = line[40..60].trim().to_string();
    let parse_from_str = NaiveDateTime::parse_from_str;
    file_creation_t = file_creation_t.replace("UTC", "").trim().to_string();
    let n_t = parse_from_str(&file_creation_t, "%Y%m%d %H%M%S")?;
    rinex_header.file_creation_t = NaiveDateTime::and_local_timezone(&n_t, Utc).unwrap();
    line.clear();

    line_len = reader.read_line(&mut line)?;
    if line_len == 0 {
        return Err(Box::new(RinexError(
            "The GNSS RINEX navigation data is expected".into(),
        )));
    }
    let mut content = line.trim();
    if content.ends_with("END OF HEADER") {
        dbg!(&rinex_header);
    } else if content.ends_with("COMMENT") {
        content = &content[..content.len() - 7];
        rinex_header.comment_line = content.to_string();
    }
    line.clear();

    line_len = reader.read_line(&mut line)?;
    if line_len == 0 {
        return Err(Box::new(RinexError(
            "The GNSS RINEX navigation data is expected".into(),
        )));
    }
    let mut content = line.trim();
    if content.ends_with("END OF HEADER") {
        dbg!(&rinex_header);
    } else if content.ends_with("IONOSPHERIC CORR") {
    }

    dbg!(rinex_header);

    Ok(HashMap::new())
}

fn check_header_option_fields<'a>(
    content: &'a str,
    r_header: &mut GnssRinexNavHeader,
) -> Option<&'a str> {
    let mut c = content;
    if c.ends_with("END OF HEADER") {
    } else if c.ends_with("COMMENT") {
        c = &c[..c.len() - 7];
        r_header.comment_line = c.to_string();
    } else if c.ends_with("IONOSPHERIC CORR") {
        let mut iono_corr: HashMap<String, String> = HashMap::new();
        iono_corr.insert(c[..5].trim().to_string(), c[5..c.len() - 16].to_string());
        r_header.iono_corr.push(iono_corr);
    } else if c.ends_with("TIME SYSTEM CORR") {
        let mut time_corr: HashMap<String, String> = HashMap::new();
        time_corr.insert(c[..5].trim().to_string(), c[5..c.len() - 16].to_string());
        r_header.time_sys_corr = time_corr;
    }
    Some(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_rinex_data() {
        if let Ok(res) = get_sats_from_rinex("BRDC00WRD_R_20233330000_01D_GN.rnx") {
            println!("OK");
        } else {
            println!("Not");
        }
    }
}
