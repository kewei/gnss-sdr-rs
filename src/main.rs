#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables)]

use std::ffi::{c_uchar, c_uint, c_void, CString};
use std::io::Error;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{env, u8};
use std::{thread, time};
use tokio::task;
use tokio::time::Duration;
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
mod rtlsdr_wrapper;
mod test_utilities;
use rtlsdr_wrapper::rtlsdr_dev_wrapper;
mod comm_func;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// C wrapper function to call the Rust callback
extern "C" {
    fn rust_callback_wrapper(buff: *mut c_uchar, buff_len: c_uint, ctx: *mut c_void);
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let sampling_rate: f32 = 2.046e6;
    let frequency: u32 = 1574.42e6 as u32;
    let freq_IF: f32 = 0.0;
    let gain = 0;
    let ppm_error = 0;
    let mut rtlsdr_dev_wrapper = rtlsdr_dev_wrapper::new();
    rtlsdr_dev_wrapper.open();
    rtlsdr_dev_wrapper.rtlsdr_config(frequency, sampling_rate as u32, gain, ppm_error);

    thread::sleep(time::Duration::from_millis(500));

    // Ctrl-C interruption
    let term = Arc::new(AtomicBool::new(true));
    let term_r = term.clone();

    ctrlc::set_handler(move || {
        term_r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    task::spawn_blocking(move || {
        rtlsdr_dev_wrapper.rtlsdr_read_async_wrapper(
            app_buffer_utilities::APP_BUFFER_NUM as u32,
            app_buffer_utilities::BUFFER_SIZE as u32,
        );
    })
    .await
    .unwrap();

    thread::sleep(time::Duration::from_millis(500));

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
        handlers.push(task::spawn_blocking(move || {
            do_data_process(
                sampling_rate,
                freq_IF,
                stage_clone,
                acq_result_clone,
                trk_result_clone,
                false,
                stop_signal_clone,
            );
        }));
        //tokio::time::sleep(Duration::from_millis(100)).await;
    }

    for handle in handlers {
        handle.await.unwrap();
    }

    Ok(())
}
