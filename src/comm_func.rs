use num::Float;
use std::cmp::PartialEq;

pub fn max_float_vec<T: Clone + PartialEq + Float>(
    vec_f: Vec<T>,
) -> Result<(T, usize), &'static str> {
    let mut ind_max = 0;
    vec_f
        .iter()
        .find(|&x| !(x.is_nan()))
        .expect("Nan in the float vector"); // Check whether there is nan in the data
    let mag_max: T = vec_f
        .clone()
        .into_iter()
        .reduce(<T as num::Float>::max)
        .expect("Empty floact vector");
    let (ind_max, _) = vec_f
        .iter()
        .enumerate()
        .find(|(ind, val)| **val == mag_max)
        .expect("Not found index of the maximum value");
    Ok((mag_max, ind_max))
}
