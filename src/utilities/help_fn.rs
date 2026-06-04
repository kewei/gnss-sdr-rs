use num_complex::Complex32;

#[inline(always)]
pub fn convert_i8_to_complex32(src: Vec<i8>) -> Vec<Complex32> {
    src.into_iter()
        .map(|x| Complex32::new(x as f32, 0.0))
        .collect()
}