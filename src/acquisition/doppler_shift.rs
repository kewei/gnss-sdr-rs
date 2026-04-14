use std::simd::f32x8;
use num_complex::Complex32;


pub struct DopplerShiftTable {
    pub doppler_freq_hz: f32,
    pub table: Vec<Complex32>,
}

impl DopplerShiftTable {
    pub fn new(doppler_freq_hz: f32, fs: f32, num_samples: usize) -> Self {
        let mut table = Vec::with_capacity(num_samples);
        let phase_step = 2.0 * std::f32::consts::PI * doppler_freq_hz / fs;

        for i in 0..num_samples {
            let phase = i as f32 * phase_step;
            table.push(Complex32::new(phase.cos(), -phase.sin())); // Negative for downconversion
        }
        Self { doppler_freq_hz, table }
    }
}


pub fn apply_doppler_shift(samples: & [Complex32], doppler_table: &DopplerShiftTable, output: &mut [Complex32]) {
    let chunks = samples.len() / 4;

    let s_ptr = samples.as_ptr() as *const f32;
    let t_ptr = doppler_table.table.as_ptr() as *const f32;
    let o_ptr = output.as_mut_ptr() as *mut f32;

    for i in 0..chunks {
        unsafe {
            let s_vec = f32x8::from_slice(std::slice::from_raw_parts(s_ptr.add(i * 8), 8));
            let t_vec = f32x8::from_slice(std::slice::from_raw_parts(t_ptr.add(i * 8), 8));
            let result_vec = multiply_simd_block(s_vec, t_vec);
            result_vec.copy_to_slice(std::slice::from_raw_parts_mut(o_ptr.add(i * 8), 8));
        }
    } 
}

/// (a + bi) * (c + di) = (ac - bd) + (ad + bc)i
pub unsafe fn multiply_simd_block(samples: f32x8, doppler_table: f32x8) -> f32x8 {
    // s = [a0, b0, a1, b1, a2, b2, a3, b3] (samples)
    // t = [c0, d0, c1, d1, c2, d2, c3, d3] (doppler_table)

    let samples_re = std::simd::simd_swizzle!(samples, [0, 0, 2, 2, 4, 4, 6, 6]); // [a0, a0, a1, a1, a2, a2, a3, a3]
    let samples_im = std::simd::simd_swizzle!(samples, [1, 1, 3, 3, 5, 5, 7, 7]); // [b0, b0, b1, b1, b2, b2, b3, b3]

    let table_re_im = doppler_table; // [c0, d0, c1, d1, c2, d2, c3, d3]
    let table_im_re = std::simd::simd_swizzle!(doppler_table, [1, 0, 3, 2, 5, 4, 7, 6]); // [d0, c0, d1, c1, d2, c2, d3, c3]

    let sign_second_part = f32x8::from_array([-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0]);
    let first_part = samples_re * table_re_im;
    let second_part = samples_im * table_im_re * sign_second_part;

    first_part + second_part
}