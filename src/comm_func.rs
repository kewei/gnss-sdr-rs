use chrono::Utc;
use num::Float;
use std::cmp::PartialEq;
use std::io::Error;

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

pub fn fectch_nav_file() -> Result<i8, Error> {
    let url_igs_folder = "https://igs.bkg.bund.de/root_ftp/IGS/BRDC/";
    let t1 = Utc::now();
    println!("{:?}", t1);
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::fectch_nav_file;

    use super::*;

    #[test]
    fn test_fetch_nav_file() {
        if let Ok(res) = fectch_nav_file() {
            assert!(true);
        } else {
            assert!(false);
        }
    }
}
