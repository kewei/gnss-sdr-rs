#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables)]

use std::ffi::{c_uchar, c_uint, c_void, CString};
use std::io::Error;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::{env, u8};
mod acquisition;
use acquisition::AcquisitionResult;
use acquisition::PRN_SEARCH_ACQUISITION_TOTAL;
mod tracking;
use tracking::TrackingResult;
mod decoding;
use decoding::nav_decoding;
mod data_process;
use crate::data_process::{do_data_process, ProcessStage};
mod app_buffer_utilities;
mod gps_ca_prn;
mod gps_constants;
mod test_utilities;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// C wrapper function to call the Rust callback
extern "C" {
    fn rust_callback_wrapper(buff: *mut c_uchar, buff_len: c_uint, ctx: *mut c_void);
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut dev_name = String::from("00000001");
    dev_name.retain(|c| c.to_digit(32).unwrap() <= 9);
    let dev_name = CString::new(dev_name).expect("CString::new failed.");
    let mut dev_index = 0;

    unsafe {
        dev_index = verbose_device_search(dev_name.into_raw());
        if dev_index == -1 {
            let dev_name = CString::new("0").expect("CString::new failed.");
            dev_index = verbose_device_search(dev_name.into_raw());
        }
    }

    if dev_index < 0 {
        panic!("Did not find supported device.")
    }

    let signal_complex = true;
    let sampling_rate: f32 = 2.046e6;
    let frequency: u32 = 1574.42e6 as u32;
    let freq_IF: f32 = 0.0;
    let mut gain = 0;
    let ppm_error = 0;
    let mut dev = ptr::null_mut();

    unsafe {
        dev = ptr::null_mut() as *mut rtlsdr_dev;
        let r = rtlsdr_open(&mut dev, dev_index as u32);
        if r < 0 {
            panic!("Failed to open rtlsdr device at {}", dev_index);
        }

        if dev.is_null() {
            panic!("Failed to open rtlsdr device at {}", dev_index);
        } else {
            verbose_set_frequency(dev, frequency);
            verbose_set_sample_rate(dev, sampling_rate as u32);
        }

        if gain == 0 {
            /* Enable automatic gain */
            verbose_auto_gain(dev);
        } else {
            /* Enable manual gain */
            gain = nearest_gain(dev, gain);
            verbose_gain_set(dev, gain);
        }

        verbose_ppm_set(dev, ppm_error);
        verbose_reset_buffer(dev); // Reset endpoint before we start reading from it (mandatory)
    }

    // Ctrl-C interruption
    let term = Arc::new(AtomicBool::new(true));
    let term_r = term.clone();

    ctrlc::set_handler(move || {
        term_r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Start RTL_SDR
    let mut r: i32 = 0;
    unsafe {
        let mut ctx = ptr::null_mut();
        r = rtlsdr_read_async(
            dev,
            Some(rust_callback_wrapper),
            ctx,
            app_buffer_utilities::RTL_BUFF_NUM as u32,
            app_buffer_utilities::BUFFER_SIZE as u32,
        );
    }

    if r < 0 {
        panic!("WARNING: RTL-SDR buffer async read failed.");
    }

    let mut acquisition_results: Vec<Arc<Mutex<AcquisitionResult>>> = Vec::new();
    let mut tracking_results: Vec<Arc<Mutex<TrackingResult>>> = Vec::new();
    let mut stages_all: Vec<Arc<Mutex<ProcessStage>>> = Vec::new();
    for i in 1..=PRN_SEARCH_ACQUISITION_TOTAL {
        let acq_result: AcquisitionResult = AcquisitionResult::new(i, sampling_rate);
        acquisition_results.push(Arc::new(Mutex::new(acq_result)));
        let trk_result = TrackingResult::new(i);
        tracking_results.push(Arc::new(Mutex::new(trk_result)));
        stages_all.push(Arc::new(Mutex::new(ProcessStage::SignalAcquisition)));
    }

    let mut handlers = Vec::new();
    for i in 0..PRN_SEARCH_ACQUISITION_TOTAL {
        let acq_result_clone = Arc::clone(&acquisition_results[i]);
        let trk_result_clone = Arc::clone(&tracking_results[i]);
        let stage_clone = Arc::clone(&stages_all[i]);
        let stop_signal_clone = Arc::clone(&term);
        handlers.push(
            thread::Builder::new()
                .name(format!("{i}").to_string())
                .spawn(move || {
                    do_data_process(
                        sampling_rate,
                        freq_IF,
                        stage_clone,
                        acq_result_clone,
                        trk_result_clone,
                        false,
                        stop_signal_clone,
                    );
                    thread::sleep(Duration::from_millis(100));
                }),
        );
    }

    for handle in handlers {
        handle.unwrap().join().unwrap();
    }

    unsafe {
        rtlsdr_close(dev);
        libc::free(dev as *mut c_void);
    }
    Ok(())
}
