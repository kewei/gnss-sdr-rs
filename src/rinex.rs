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
    leap_sec: String,
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
            leap_sec: "".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit1 {
    iode: f32,
    crs: f32,
    delta_n: f32,
    m0: f32,
}

impl BroadCastOrbit1 {
    fn new() -> Self {
        Self {
            iode: 0.0,
            crs: 0.0,
            delta_n: 0.0,
            m0: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit2 {
    cuc: f32,
    e_eccentricity: f32,
    cus: f32,
    sqrt_a: f32,
}

impl BroadCastOrbit2 {
    fn new() -> Self {
        Self {
            cuc: 0.0,
            e_eccentricity: 0.0,
            cus: 0.0,
            sqrt_a: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit3 {
    toe: f32,
    cic: f32,
    omega0: f32,
    cis: f32,
}

impl BroadCastOrbit3 {
    fn new() -> Self {
        Self {
            toe: 0.0,
            cic: 0.0,
            omega0: 0.0,
            cis: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit4 {
    i0: f32,
    crc: f32,
    omega: f32,
    omega_dot: f32,
}

impl BroadCastOrbit4 {
    fn new() -> Self {
        Self {
            i0: 0.0,
            crc: 0.0,
            omega: 0.0,
            omega_dot: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit5 {
    idot: f32,
    code_on_l2: f32,
    gps_week: f32,
    l2_p_flag: f32,
}

impl BroadCastOrbit5 {
    fn new() -> Self {
        Self {
            idot: 0.0,
            code_on_l2: 0.0,
            gps_week: 0.0,
            l2_p_flag: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit6 {
    sv_accuracy: f32,
    sv_health: f32,
    tgd: f32,
    iodc: f32,
}

impl BroadCastOrbit6 {
    fn new() -> Self {
        Self {
            sv_accuracy: 0.0,
            sv_health: 0.0,
            tgd: 0.0,
            iodc: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct BroadCastOrbit7 {
    t_transmission_message: f32,
    fit_interval_hours: f32,
}

impl BroadCastOrbit7 {
    fn new() -> Self {
        Self {
            t_transmission_message: 0.0,
            fit_interval_hours: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct GnssRinexNavRecord {
    satellite_sys: String,
    satellite_number: u16,
    time: DateTime<Utc>,
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

impl GnssRinexNavRecord {
    fn new() -> Self {
        Self {
            satellite_sys: "".to_string(),
            satellite_number: 0,
            time: Utc::now(),
            sv_clock_bias: 0.0,
            sv_clock_drift: 0.0,
            sv_clocl_drift_rate: 0.0,
            orbit1: BroadCastOrbit1::new(),
            orbit2: BroadCastOrbit2::new(),
            orbit3: BroadCastOrbit3::new(),
            orbit4: BroadCastOrbit4::new(),
            orbit5: BroadCastOrbit5::new(),
            orbit6: BroadCastOrbit6::new(),
            orbit7: BroadCastOrbit7::new(),
        }
    }
}
pub struct GpsRinexNavData {
    rinex_header: GnssRinexNavHeader,
    rinex_data_record: Vec<GnssRinexNavRecord>,
}

impl GpsRinexNavData {
    fn new() -> Self {
        Self {
            rinex_header: GnssRinexNavHeader::new(),
            rinex_data_record: Vec::new(),
        }
    }
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

    loop {
        line_len = reader.read_line(&mut line)?;
        if line_len == 0 {
            return Err(Box::new(RinexError(
                "The GNSS RINEX navigation data is expected".into(),
            )));
        }
        if let Some(content) = check_header_option_fields(&line.trim(), &mut rinex_header) {
            if content == "END OF HEADER" {
                line.clear();
                break;
            }
        }
        line.clear();
    }

    let mut n = 0;
    let mut rinex_nav_data = GpsRinexNavData::new();
    rinex_nav_data.rinex_header = rinex_header;
    loop {
        line_len = reader.read_line(&mut line)?;
        if line_len == 0 {
            break;
        }
        let mut rinex_record = GnssRinexNavRecord::new();
        if let Some(content) = get_rinex_nav_record(n % 8, &line, &mut rinex_record) {
            rinex_nav_data.rinex_data_record.push(rinex_record);
        }
        n += 1;
        line.clear();
    }

    if n % 8 != 0 {
        return Err(Box::new(RinexError(
            "Rinex data record is not complete".into(),
        )));
    }

    dbg!(rinex_nav_data.rinex_header);
    dbg!(&rinex_nav_data.rinex_data_record[2]);

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
    } else if c.ends_with("LEAP SECONDS") {
        r_header.leap_sec = c.strip_suffix("LEAP SECONDS")?.trim().to_string();
    }
    Some(content)
}

fn get_rinex_nav_record<'a>(
    n: u32,
    content: &'a str,
    r_record: &mut GnssRinexNavRecord,
) -> Option<&'a str> {
    let mut c = content;
    let parse_from_str = NaiveDateTime::parse_from_str;
    match n {
        0 => {
            r_record.satellite_sys = c[0..1].to_string();
            r_record.satellite_number = c[1..3].parse().expect("Parsing string to number");
            let dt = parse_from_str(c[3..23].trim(), "%Y %m %d %H %M %S")
                .expect("Parsing string to DateTime");
            r_record.time = NaiveDateTime::and_local_timezone(&dt, Utc).unwrap();
            r_record.sv_clock_bias = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for SV clock bias");
            r_record.sv_clock_drift = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for SV clock drift");
            r_record.sv_clocl_drift_rate = c[61..61 + 19]
                .trim()
                .parse()
                .expect("Parsing error for SV clock drift rate");
        }
        1 => {
            r_record.orbit1.iode = c[4..4 + 19].trim().parse().expect("Parsing error for IODE");
            r_record.orbit1.crs = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Crs");
            r_record.orbit1.delta_n = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Delta n");
            r_record.orbit1.m0 = c[61..61 + 19].trim().parse().expect("Parsing error for M0");
        }
        2 => {
            r_record.orbit2.cuc = c[4..4 + 19].trim().parse().expect("Parsing error for Cuc");
            r_record.orbit2.e_eccentricity = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Eccentricity");
            r_record.orbit2.cus = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Cus");
            r_record.orbit2.sqrt_a = c[61..61 + 19]
                .trim()
                .parse()
                .expect("Parsing error for sqrt(A)");
        }
        3 => {
            r_record.orbit3.toe = c[4..4 + 19].trim().parse().expect("Parsing error for Toe");
            r_record.orbit3.cic = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Cic");
            r_record.orbit3.omega0 = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for OMEGA0");
            r_record.orbit3.cis = c[61..61 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Cis");
        }
        4 => {
            r_record.orbit4.i0 = c[4..4 + 19].trim().parse().expect("Parsing error for i0");
            r_record.orbit4.crc = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Crc");
            r_record.orbit4.omega = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for omega");
            r_record.orbit4.omega_dot = c[61..61 + 19]
                .trim()
                .parse()
                .expect("Parsing error for OMEGA DOT");
        }
        5 => {
            r_record.orbit5.idot = c[4..4 + 19].trim().parse().expect("Parsing error for IDOT");
            r_record.orbit5.code_on_l2 = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Codes on L2");
            r_record.orbit5.gps_week = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for GPS Week");
            r_record.orbit5.l2_p_flag = c[61..61 + 19]
                .trim()
                .parse()
                .expect("Parsing error for L2 P data flag");
        }
        6 => {
            r_record.orbit6.sv_accuracy = c[4..4 + 19]
                .trim()
                .parse()
                .expect("Parsing error for SV accuracy");
            r_record.orbit6.sv_health = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for SV health");
            r_record.orbit6.tgd = c[42..42 + 19]
                .trim()
                .parse()
                .expect("Parsing error for TGD");
            r_record.orbit6.iodc = c[61..61 + 19]
                .trim()
                .parse()
                .expect("Parsing error for IODC");
        }
        7 => {
            r_record.orbit7.t_transmission_message = c[4..4 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Transmission time");
            r_record.orbit7.fit_interval_hours = c[23..23 + 19]
                .trim()
                .parse()
                .expect("Parsing error for Fit Interval");
        }
        _ => {
            return None;
        }
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
