use plotpy::{Curve, Plot, StrError};
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use std::{thread, time};

const EPSILON: f64 = 1e-8;
const MAX_ITER: usize = 100;

pub fn plot_psd(samples: &[f32], fs: u32) -> Result<(), StrError> {
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
