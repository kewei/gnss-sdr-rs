use crate::constants::gps_ca_constants::GPS_CA_CODE_32_PRN;

/// Generate CA code samples for 32 PRN code based on sampling frequency which might not be multiples of CA code rate
pub fn generate_ca_code_samples(prn: usize, f_sampling: f32) -> Vec<i8> {
    let num_samples = (f_sampling
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let samples_ind: Vec<usize> = (0..num_samples)
        .map(|x| {
            (x as f32 * gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / f_sampling).floor()
                as usize
        })
        .collect();

    let ca_code = GPS_CA_CODE_32_PRN[prn];
    
    samples_ind.iter().map(|&ind| ca_code[ind]).collect()
}