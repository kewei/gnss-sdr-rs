use chrono::{Datelike, Utc};
use num::Float;
use scraper::{Html, Selector};
use std::cmp::PartialEq;
use std::error::Error;

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

pub fn fetch_nav_file() -> Result<String, Box<dyn Error>> {
    let url_igs_folder = "https://igs.bkg.bund.de/root_ftp/IGS/BRDC/";
    let t1 = Utc::now();
    let year = t1.year();
    let day_year = t1.ordinal();
    let url_folder_rinex =
        url_igs_folder.to_owned() + &year.to_string() + "/" + &day_year.to_string();
    let http_response = reqwest::blocking::get(url_folder_rinex.to_owned())?.text()?;
    let http_parse = Html::parse_document(&http_response);
    let td_selector = Selector::parse("td").unwrap();
    let a_selector = Selector::parse("a").unwrap();
    let mut file_url = "";
    for td_element in http_parse.select(&td_selector) {
        if let Some(f) = td_element.select(&a_selector).next() {
            file_url = f.attr("href").unwrap();
            if file_url.ends_with("GN.rnx.gz") {
                break;
            }
        }
    }
    if file_url.is_empty() {
        panic!("Could not download GPS Navigation RINEX file from https://igs.bkg.bund.de/");
    }

    Ok((url_folder_rinex + file_url).to_string())
}

#[cfg(test)]
mod tests {
    use super::fetch_nav_file;

    //#[tokio::test]
    #[test]
    fn test_fetch_nav_file() {
        match fetch_nav_file() {
            Ok(res) => {
                println!("{:#?}", res);
            }
            Err(e) => {
                println!("Wrong");
                dbg!(e);
            }
        };
    }
}
