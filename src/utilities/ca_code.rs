use crate::constants::gps_ca_constants::GPS_CA_CODE_32_PRN;
use crate::constants::gps_def_constants;

/// Generate CA code samples for 32 PRN code based on sampling frequency which might not be multiples of CA code rate
/// # Arguments
/// * `prn` - PRN number of the satellite (0-31)
/// * `code_rate` - Code rate of the CA code (e.g., 1.023e6 for GPS L1 C/A code), could be different during tracking
/// * `f_sampling` - Sampling frequency (e.g., 4.092e6 for 4x oversampling of GPS L1 C/A code)
/// # Returns
/// A vector of i8 representing the CA code samples (1 or -1)
#[inline(always)]
pub fn generate_ca_code_samples(prn: u8, code_rate: f32, f_sampling: f32) -> Vec<i8> {
    let num_samples = (f_sampling
        / (code_rate
            / gps_def_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let samples_ind: Vec<usize> = (0..num_samples)
        .map(|x| {
            (x as f32 * code_rate / f_sampling).floor()
                as usize
        })
        .collect();

    let ca_code = GPS_CA_CODE_32_PRN[prn as usize];
    
    samples_ind.iter().map(|&ind| ca_code[ind]).collect()
}