use itertools::izip;
use rustfft::num_complex::Complex32;
use std::error::Error;
use std::f32::consts::PI;

use crate::acquisition::AcquistionStatistics;
use crate::gps_ca_prn::generate_ca_code;
use crate::gps_constants;

static DLL_DUMPING_RATIO: f32 = 0.7;
static PLL_DUMPING_RATIO: f32 = 0.7;
static DLL_NOISE_BANDWIDTH: f32 = 2.0;
static PLL_NOISE_BANDWIDTH: f32 = 25.0;
// Summation interval
static PLL_SUM_CARR: f32 = 0.001;
static DLL_SUM_CODE: f32 = 0.001;
static PLL_GAIN: f32 = 0.25;
static DLL_GAIN: f32 = 1.0;
static EARLY_LATE_SPACE: f32 = 0.5;

pub struct TrackingStatistics {
    i_prompt: Vec<f32>,
    q_prompt: Vec<f32>,
    i_early: Vec<f32>,
    q_early: Vec<f32>,
    i_late: Vec<f32>,
    q_late: Vec<f32>,
    code_error: Vec<f32>,
    code_error_filtered: Vec<f32>,
    carrier_erorr: Vec<f32>,
    carrier_error_filtered: Vec<f32>,
    carrier_freq: Vec<f32>,
    code_freq: Vec<f32>,
}

pub fn do_track(
    signal_input: Vec<i16>,
    acq_results: Vec<AcquistionStatistics>,
    f_sampling: f32,
    f_IF: f32,
) -> Result<TrackingStatistics, Box<dyn Error>> {
    let samples_iq: Vec<Complex32> = signal_input
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect(); // Duplicated

    for acq_result in acq_results {
        let num_ca_code_samples = (f_sampling
            / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
                / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
            .round() as usize; // Duplicated

        let prn = acq_result.prn;
        let doppler_freq = acq_result.doppler_freq;
        let code_phase = acq_result.code_phase;
        let freq = f_IF + doppler_freq;

        let code_freq: f32 = gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S;
        let code_phase: f32 = 0.0;
        let freq: f32 = f_IF + doppler_freq;
        let carrier_phase_error: f32 = 0.0;
        let code_nco: f32 = 0.0;
        let code_error: f32 = 0.0;
        let carrier_nco: f32 = 0.0;
        let carrier_error: f32 = 0.0;

        let code_phase_step: f32 = code_freq / f_sampling;
        let num_ca_code_samples = ((gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS - code_phase)
            / code_phase_step)
            .ceil() as usize;

        // Duplicated
        let mut ca_code = generate_ca_code(prn as usize);
        ca_code.insert(
            0,
            ca_code[gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize - 1],
        );
        ca_code.push(ca_code[0]);

        let ca_code_prompt: Vec<f32> = (0..num_ca_code_samples)
            .map(|x| ca_code[(x as f32 * code_phase_step + code_phase).ceil() as usize + 1] as f32)
            .collect();

        let (
            carrier_freq,
            q_prompt,
            i_prompt,
            carrier_error,
            carrier_nco,
            carrier_phase_error,
            q_arm,
            i_arm,
        ) = costas_loop(
            &samples_iq,
            ca_code_prompt,
            freq,
            carrier_error,
            carrier_phase_error,
            carrier_nco,
            code_freq,
            code_phase,
            f_sampling,
        );

        let (
            ca_code_prompt,
            code_freq,
            d_code_error,
            code_nco,
            code_phase,
            i_early,
            q_early,
            i_late,
            q_late,
            i_prompt,
            q_prompt,
        ) = dll_early_late(
            q_arm, i_arm, code_freq, code_phase, code_error, code_nco, ca_code, f_sampling,
        );
    }
    todo!();
}

fn costas_loop(
    signal_samples: &Vec<Complex32>,
    prompt_code_samples: Vec<f32>,
    freq: f32,
    carrier_error: f32,
    carrier_phase_error: f32,
    carrier_nco: f32,
    code_freq: f32,
    code_phase: f32,
    f_sampling: f32,
) -> (f32, f32, f32, f32, f32, f32, Vec<f32>, Vec<f32>) {
    let num_ca_code_samples = ((gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS - code_phase)
        / (code_freq / f_sampling))
        .ceil() as usize;
    let local_carrier: Vec<Complex32> = (0..num_ca_code_samples)
        .map(|x| {
            Complex32::new(
                (2.0 * PI * freq / f_sampling * x as f32 + carrier_phase_error).cos(),
                (2.0 * PI * freq / f_sampling * x as f32 + carrier_phase_error).sin(),
            )
        })
        .collect();
    let carrier_phase_error = (2.0 * PI * freq / f_sampling * num_ca_code_samples as f32
        + carrier_phase_error)
        % (2.0 * PI);

    let (q_arm, i_arm): (Vec<f32>, Vec<f32>) = local_carrier
        .iter()
        .zip(signal_samples.iter())
        .map(|(x, y)| ((x * y).re, (x * y).im))
        .unzip();
    let (q_prompt, i_prompt): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, prompt_code_samples)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_prompt: f32 = q_prompt.iter().sum();
    let i_prompt: f32 = i_prompt.iter().sum();
    let d_carrier_error = (q_prompt / i_prompt).atan() / (2.0 * PI);

    let (tau1_carr, tau2_carr) =
        calculate_loop_efficient(PLL_NOISE_BANDWIDTH, PLL_DUMPING_RATIO, PLL_GAIN);

    let carrier_nco = carrier_nco
        + (tau2_carr / tau1_carr) * (d_carrier_error - carrier_error)
        + d_carrier_error * (PLL_SUM_CARR / tau1_carr);

    // Update carrier frequency
    let carrier_freq = freq + carrier_nco;

    (
        carrier_freq,
        q_prompt,
        i_prompt,
        carrier_error,
        carrier_nco,
        carrier_phase_error,
        q_arm,
        i_arm,
    )
}

fn dll_early_late(
    q_arm: Vec<f32>,
    i_arm: Vec<f32>,
    code_freq: f32,
    code_phase: f32,
    old_code_error: f32,
    code_nco: f32,
    ca_code: Vec<i32>,
    f_sampling: f32,
) -> (Vec<f32>, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32) {
    let code_phase_step: f32 = code_freq / f_sampling;
    let num_ca_code_samples = ((gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS - code_phase)
        / code_phase_step)
        .ceil() as usize;
    let ca_code_early: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| {
            ca_code
                [(x as f32 * code_phase_step + code_phase - EARLY_LATE_SPACE).ceil() as usize + 1]
                as f32
        })
        .collect();
    let ca_code_late: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| {
            ca_code
                [(x as f32 * code_phase_step + code_phase + EARLY_LATE_SPACE).ceil() as usize + 1]
                as f32
        })
        .collect();
    let ca_code_prompt: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| ca_code[(x as f32 * code_phase_step + code_phase).ceil() as usize + 1] as f32)
        .collect();
    let code_phase =
        (num_ca_code_samples - 1) as f32 * code_phase_step + code_phase + code_phase_step
            - gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS;

    let (q_early, i_early): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, ca_code_early)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_early: f32 = q_early.iter().sum();
    let i_early: f32 = i_early.iter().sum();
    let (q_late, i_late): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, ca_code_late)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_late: f32 = q_late.iter().sum();
    let i_late: f32 = i_late.iter().sum();
    let (q_prompt, i_prompt): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, &ca_code_prompt)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_prompt: f32 = q_prompt.iter().sum();
    let i_prompt: f32 = i_prompt.iter().sum();

    let d_code_error: f32 = ((i_early.powf(2.0) + q_early.powf(2.0)).sqrt()
        - (i_late.powf(2.0) + q_late.powf(2.0)).sqrt())
        / ((i_early.powf(2.0) + q_early.powf(2.0)).sqrt()
            + (i_late.powf(2.0) + q_late.powf(2.0)).sqrt());
    let (tau1_code, tau2_code) =
        calculate_loop_efficient(DLL_NOISE_BANDWIDTH, DLL_DUMPING_RATIO, DLL_GAIN);
    let code_nco = code_nco
        + (tau2_code / tau1_code) * (d_code_error - old_code_error)
        + d_code_error * (DLL_SUM_CODE / tau1_code);
    let code_freq: f32 = code_freq - code_nco;
    (
        ca_code_prompt,
        code_freq,
        d_code_error,
        code_nco,
        code_phase,
        i_early,
        q_early,
        i_late,
        q_late,
        i_prompt,
        q_prompt,
    )
}

fn calculate_loop_efficient(noise_bw: f32, dumping_ratio: f32, gain: f32) -> (f32, f32) {
    let w: f32 = noise_bw * 8.0 * dumping_ratio / (4.0 * dumping_ratio.powf(2.0) + 1.0);
    let tau1: f32 = gain / (w * w);
    let tau2: f32 = 2.0 * dumping_ratio / w;
    (tau1, tau2)
}
