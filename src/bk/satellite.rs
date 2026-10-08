use std::f64::consts::PI;

struct EphemerisData {
    toe: f64,
    m0: f64,
    delta_n: f64,
    e_eccentricity: f64,
    sqrt_a: f64,
    omega0: f64,
    i0: f64,
    omega: f64,
    cuc: f64,
    cus: f64,
    crc: f64,
    crs: f64,
    cic: f64,
    cis: f64,
}

pub fn cal_satellite_pos(ephemeris: &EphemerisData, t_receiver: f64) -> (f64, f64, f64, f64) {
    const GM: f64 = 3.986005e14;

    let toe = ephemeris.toe;
    let M0 = ephemeris.m0;
    let delta_n = ephemeris.delta_n;
    let ecc = ephemeris.e_eccentricity;
    let sqrtA = ephemeris.sqrt_a;
    let Omega0 = ephemeris.Omega0;
    let i0 = ephemeris.i0;
    let omega = ephemeris.omega;
    let Cuc = ephemeris.cuc;
    let Cus = ephemeris.cus;
    let Crc = ephemeris.crc;
    let Crs = ephemeris.crs;
    let Cic = ephemeris.cic;
    let Cis = ephemeris.cis;

    let tk = t_receiver - toe;
    let tk = if tk > 302400.0 {
        tk - 604800.0
    } else if tk < -302400.0 {
        tk + 604800.0
    } else {
        tk
    };

    let n0 = (GM / sqrtA.powi(3)).sqrt();
    let n = n0 + delta_n;

    // Mean anomaly
    let M = M0 + n * tk;

    let mut E = M;
    for _ in 0..10 {
        let E_prev = E;
        E = M + ecc * E.sin();
        if (E - E_prev).abs() < 1.0e-12 {
            break;
        }
    }

    // True anomaly
    let v = ((1.0 - ecc.powi(2)).sqrt() * E.sin()).atan2(E.cos() - ecc);

    // Argument of latitude
    let phi = v + omega;

    let delta_u = Cuc * (2.0 * phi).cos() + Cus * (2.0 * phi).sin();
    let delta_r = Crc * (2.0 * phi).cos() + Crs * (2.0 * phi).sin();
    let delta_i = Cic * (2.0 * phi).cos() + Cis * (2.0 * phi).sin();

    // Corrected orbital elements
    let u = phi + delta_u;
    let r = sqrtA.powi(2) * (1.0 - ecc * E.cos()) + delta_r;
    let i = i0 + delta_i;

    // Coordinates in orbital plane
    let x_orbital = r * u.cos();
    let y_orbital = r * u.sin();

    // Earth-fixed coordinates (ECEF)
    let x = x_orbital * Omega0.cos() - y_orbital * i.cos() * Omega0.sin();
    let y = x_orbital * Omega0.sin() + y_orbital * i.cos() * Omega0.cos();
    let z = y_orbital * i.sin();

    // Relativistic correction (approximation)
    let delta_t_rel = -2.0 * (GM / C.powi(2)) * ecc * E.sin();

    // Apply relativistic correction to the satellite clock time
    let corrected_satellite_time = receiver_time - delta_t_rel;

    (x, y, z, corrected_satellite_time)
}
