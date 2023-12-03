use chrono::DateTime;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

pub struct GnssRinexNavHeader {
    rinext_v: String,
    file_type: String,
    system: String,
    file_creation_t: String,
    comment_line: String,
    iono_corr: Vec<HashMap<String, HashMap<String, String>>>,
    time_sys_corr: HashMap<String, HashMap<String, String>>,
    leap_sec: u8,
}

impl GnssRinexNavHeader {
    fn new() -> Self {
        Self {
            rinex_v: "",
            file_type: "",
            system: "",
            file_creation_t: "",
            comment_line: "",
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
    time: DateTime,
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

pub fn get_sats_from_rinex(file_name: &str) -> Result<HashMap, Box<dyn Error>> {
    let rinex_header = GnssRinexNavHeader::new();
    let file = File::open(file_name)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut line_len = reader.read_line(&mut line)?;
    if line_len == 0 {
        return Err("The GNSS RINEX navigation data is expected");
    }

    rinex_header.rinex_v = line[0..9].trim().parse();
    rinex_header.file_type = line[9..29].trim();
    rinex_header.system = line[29..49].trim();
    dbg!(rinex_header);
    Ok(HashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_rinex_data() {
        if let Ok(res) = get_sats_from_rinex("BRDC00WRD_R_20233330000_01D_GN.rnx") {
            println("OK");
        } else {
            println("Not");
        }
    }
}
