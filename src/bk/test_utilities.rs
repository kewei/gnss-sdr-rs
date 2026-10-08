use byteorder::{BigEndian, ReadBytesExt};
use plotpy::{Curve, Plot};
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Result};
use std::sync::Arc;
use std::{thread, time};

use crate::app_buffer_utilities;
use app_buffer_utilities::{APPBUFF, APP_BUFFER_NUM, BUFFER_SIZE};

const EPSILON: f64 = 1e-8;
const MAX_ITER: usize = 100;

pub fn plot_psd(samples: &[f32], fs: u32) -> Result<()> {
    let hann_window = hann_window(samples);
    // calc spectrum
    let spectrum_hann_window = samples_fft_to_spectrum(
        // (windowed) samples
        &hann_window,
        // sampling rate
        fs,
        // optional frequency limit: e.g. only interested in frequencies 50 <= f <= 150?
        FrequencyLimit::All,
        // optional scale
        Some(&divide_by_N_sqrt),
    )
    .unwrap();

    let samples_n: Vec<f32> = (0..samples.len() as u16).map(|x| f32::from(x)).collect();
    let (freq_vec_t, ampl_vec_t): (Vec<_>, Vec<_>) =
        spectrum_hann_window.data().iter().cloned().unzip();

    let freq_vec: Vec<f32> = freq_vec_t.iter().map(|x| x.val() / 1000.0).collect();
    let ampl_vec: Vec<f32> = ampl_vec_t.iter().map(|x| 10.0 * x.val().log10()).collect();
    //let ampl_vec: Vec<f32> = ampl_vec_t.iter().map(|x| x.val()).collect();

    let mut curve1 = Curve::new();
    let mut curve2 = Curve::new();

    curve1.draw(&samples_n, &samples.to_vec());
    curve2.draw(&freq_vec, &ampl_vec);
    let mut plot = Plot::new();
    plot.set_super_title("Input signal").set_gaps(0.1, 0.1);
    plot.set_figure_size_inches(8.0, 5.0);
    plot.set_subplot(2, 1, 1)
        .set_title("Signal samples")
        .add(&curve1)
        .grid_labels_legend("n", "samples")
        .set_equal_axes(true);

    plot.set_subplot(2, 1, 2)
        .set_title("PSD")
        .add(&curve2)
        .grid_labels_legend("frequency/KHz", "Amplitude/dB")
        .set_equal_axes(true);

    plot.save_and_show("doc_plot.svg");
    print!("I have finished plotting, now waiting for 5 seconds ...");
    let five_sec = time::Duration::from_secs(5);
    thread::sleep(five_sec);
    Ok(())
}

pub fn plot_samples(samples: &[f32]) {
    let samples_n: Vec<f32> = (0..samples.len() as u16).map(|x| f32::from(x)).collect();
    let mut curve1 = Curve::new();
    curve1.draw(&samples_n, &samples.to_vec());

    let mut plot = Plot::new();
    plot.set_figure_size_inches(8.0, 5.0);
    plot.set_title("Signal samples")
        .add(&curve1)
        .grid_labels_legend("n", "samples")
        .set_equal_axes(true);

    plot.save_and_show("samples.svg");
}

///
/// - f_name: file path
pub fn read_data_file(f_name: &str) -> Result<()> {
    let f = File::open(f_name)?;
    const data_type: usize = 0; // 0: real, 1: complex
    let mut buff_read = BufReader::new(f);
    const buf_size: usize = if data_type == 0 {
        BUFFER_SIZE
    } else {
        2 * BUFFER_SIZE
    };
    let values1 = [0i32; BUFFER_SIZE];
    let mut values2 = [0; buf_size];
    loop {
        let mut values: Vec<i8> = Vec::with_capacity(2 * BUFFER_SIZE);
        if !(buff_read
            .fill_buf()
            .expect("Filling buffer has an error!")
            .is_empty())
        {
            buff_read.read_exact(&mut values2[..])?;
        }

        let mut t_v: Vec<(i32, i32)> = Vec::new();
        if data_type == 0 {
            t_v = values2
                .into_iter()
                .map(|x| x as i32)
                .zip(values1.into_iter())
                .collect();
        } else {
            t_v = values2
                .chunks_exact(2)
                .map(|x| (x[0] as i32, x[1] as i32))
                .collect();
        }

        values = self_flatten(&t_v).iter().map(|&x| x as i8).collect(); // Be careful!

        let app_buffer_clone = unsafe { Arc::clone(&APPBUFF) };
        let mut app_buffer_val = app_buffer_clone
            .write()
            .expect("Error in locking when incrementing buff_cnt of AppBuffer");

        // Copy data
        let cnt = app_buffer_val.buff_cnt % APP_BUFFER_NUM;
        app_buffer_val
            .app_buffer
            .write_latest(&values[..], (cnt * 2 * BUFFER_SIZE) as isize);

        app_buffer_val.buff_cnt += 1;
        // println!("cnt: {}", app_buffer_val.buff_cnt);

        if app_buffer_val.buff_cnt % 30720 == 0 {
            let one_thousand_ms = time::Duration::from_millis(1000);
            thread::sleep(one_thousand_ms);
        }
    }

    Ok(())
}

#[no_mangle]
#[inline(never)]
fn self_flatten(data: &[(i32, i32)]) -> &[i32] {
    use std::mem::transmute;
    use std::slice::from_raw_parts;
    unsafe { transmute(from_raw_parts(data.as_ptr(), data.len() * 2)) }
}
