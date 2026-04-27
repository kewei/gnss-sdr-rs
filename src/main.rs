// #![allow(non_upper_case_globals)]
// #![allow(non_camel_case_types)]
// #![allow(non_snake_case)]
// #![allow(unused_variables)]

// use crossbeam_channel::{unbounded, Sender};
// use std::ffi::{c_uchar, c_uint, c_void, CString};
// use std::fmt::format;
// use std::io::Error;
// use std::ptr;
// use std::sync::atomic::{AtomicBool, Ordering};
// use std::sync::{Arc, Mutex};
// use std::{env, u8};
// use std::{thread, time};
// use tokio::task;
// use tokio::time::Duration;
// mod acquisition;
// mod rinex;
// use acquisition::AcquisitionResult;
// use acquisition::PRN_SEARCH_ACQUISITION_TOTAL;
// mod tracking;
// use tracking::TrackingResult;
// mod decoding;
// use decoding::{nav_decoding, NavSyncStatus};
// mod data_process;
// use data_process::{do_data_process, ProcessStage};
// mod app_buffer_utilities;
// mod gps_ca_prn;
// mod gps_constants;
// mod view;
// use view::{data_view, NavigationView};
// mod rtlsdr_wrapper;
// mod test_utilities;
// use rtlsdr_wrapper::rtlsdr_dev_wrapper;
// mod comm_func;

// include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// // C wrapper function to call the Rust callback
// extern "C" {
//     fn rust_callback_wrapper(buff: *mut c_uchar, buff_len: c_uint, ctx: *mut c_void);
// }

// //#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
// fn main() -> Result<(), Error> {
//     let sampling_rate: f32 = 2.0e6;
//     let frequency: u32 = 1574.42e6 as u32;
//     let freq_IF: f32 = 0.0;
//     let gain = 70;
//     let ppm_error = 0;
//     let mut rtlsdr_dev_wrapper = rtlsdr_dev_wrapper::new();
//     rtlsdr_dev_wrapper.open();
//     rtlsdr_dev_wrapper.rtlsdr_config(frequency, sampling_rate as u32, gain, ppm_error);

//     let (m_sender, m_receiver) = unbounded::<NavigationView>();

//     thread::sleep(time::Duration::from_millis(500));

//     // Ctrl-C interruption
//     let term = Arc::new(AtomicBool::new(true));
//     let term_r = term.clone();

//     ctrlc::set_handler(move || {
//         term_r.store(false, Ordering::SeqCst);
//     })
//     .expect("Error setting Ctrl-C handler");

//     let mut handlers = Vec::new();

//     let stop_signal_clone = Arc::clone(&term);
//     handlers.push(
//         thread::Builder::new()
//             .name("Device reader".to_string())
//             .spawn(move || {
//                 rtlsdr_dev_wrapper.rtlsdr_read_async_wrapper(
//                     app_buffer_utilities::APP_BUFFER_NUM as u32,
//                     app_buffer_utilities::BUFFER_SIZE as u32,
//                     stop_signal_clone,
//                 );
//             })
//             .unwrap(),
//     );

//     thread::sleep(time::Duration::from_millis(500));

//     handlers.push(
//         thread::Builder::new()
//             .name("Plotting thread".to_string())
//             .spawn(move || {
//                 data_view(m_receiver);
//             })
//             .unwrap(),
//     );

//     let mut acquisition_results: Vec<Arc<Mutex<AcquisitionResult>>> = Vec::new();
//     let mut tracking_results: Vec<Arc<Mutex<TrackingResult>>> = Vec::new();
//     let mut stages_all: Vec<Arc<Mutex<ProcessStage>>> = Vec::new();
//     let mut nav_stats_all: Vec<Arc<Mutex<NavSyncStatus>>> = Vec::new();
//     for i in 1..=PRN_SEARCH_ACQUISITION_TOTAL {
//         let acq_result: AcquisitionResult = AcquisitionResult::new(i, sampling_rate);
//         acquisition_results.push(Arc::new(Mutex::new(acq_result)));
//         let trk_result = TrackingResult::new(i);
//         tracking_results.push(Arc::new(Mutex::new(trk_result)));
//         let nav_stat = NavSyncStatus::new();
//         nav_stats_all.push(Arc::new(Mutex::new(nav_stat)));
//         stages_all.push(Arc::new(Mutex::new(ProcessStage::SignalAcquisition)));
//         let nav_view = NavigationView::new(i);
//     }

//     let mut cnt_all: Vec<Arc<Mutex<usize>>> = Vec::with_capacity(PRN_SEARCH_ACQUISITION_TOTAL);
//     (0..PRN_SEARCH_ACQUISITION_TOTAL).for_each(|_| cnt_all.push(Arc::new(Mutex::new(0))));

//     for i in 0..PRN_SEARCH_ACQUISITION_TOTAL {
//         let acq_result_clone = Arc::clone(&acquisition_results[i]);
//         let trk_result_clone = Arc::clone(&tracking_results[i]);
//         let nav_stat_clone = Arc::clone(&nav_stats_all[i]);
//         let stage_clone = Arc::clone(&stages_all[i]);
//         let term_signal_clone = Arc::clone(&term);
//         let cnt_each = Arc::clone(&cnt_all[i]);
//         let sender_clone = m_sender.clone();
//         handlers.push(
//             thread::Builder::new()
//                 .name(format!("PRN: {i}").to_string())
//                 .spawn(move || {
//                     do_data_process(
//                         i,
//                         sampling_rate,
//                         freq_IF,
//                         stage_clone,
//                         acq_result_clone,
//                         trk_result_clone,
//                         nav_stat_clone,
//                         false,
//                         cnt_each,
//                         sender_clone,
//                         term_signal_clone,
//                     );
//                 })
//                 .unwrap(),
//         );
//     }

//     for handle in handlers {
//         handle.join().unwrap();
//         thread::sleep(Duration::from_millis(50));
//     }

//     Ok(())
// }

use gnss_sdr_rs::acquisition::do_acquisition::AcquisitionResult;
use gnss_sdr_rs::tracking::do_tracking::{TrackingManager, TrackingMessage};
use serde_json::json;
use std::ops::Mul;
use std::sync::Arc;
use std::thread;
use crate::rf::samples_buffer::{BUFFER_SIZE, SampleComplex, SampleReal, SamplesRingBuffer, create_samples_ring_buffer};
use crate::rf::rf_thread::rf_thread;
use crate::sdr_store::sdr_wrapper::SdrDeviceWrapper;
use crate::sdr_store::sdr_wrapper::start_device_with_name;
use crate::sdr_store::sdr_thread::sdr_thread;
use crate::config::app_config::{AppConfig, APP_CONFIG_FILE};
use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
use crate::acquisition::acquisition::do_acquisition;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("----------- GNSS-SDR-RS started -------------");

    // Load the application configuration
    let app_config = AppConfig::from_toml_file(APP_CONFIG_FILE)?;
    println!("Starting stream with device: {:?}", app_config.device);

    let mut sdr_dev = start_device_with_name(app_config.device, None)?;
    sdr_dev.config(json!(&app_config.sdr))?;

    let raw_ring_buffer: SamplesRingBuffer = create_samples_ring_buffer::<SampleComplex>(BUFFER_SIZE);
    
    /// We use a large buffer to store the samples from RF thread, and then the acquisition and tracking threads 
    /// can read from it. Here only the RF thread will write to the buffer, and the acquisition and tracking threads 
    /// will read from it, so we don't need to worry about concurrent write and read.
    let multicast_buffer: Arc<MulticastRingBuffer> = Arc::new(MulticastRingBuffer::new(1 << 20));  // 1M samples
    let (tx_acq, rx_acq) = crossbeam::channel::unbounded::<AcquisitionResult>();
    let (tx_trk, rx_trk) = crossbeam::channel::unbounded::<TrackingMessage>();

    thread::spawn(move || {
        sdr_thread(&mut sdr_dev, &mut raw_ring_buffer.producer);
    }).join()?;

    let rf_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        rf_thread(&app_config.rfconfig, &sdr_dev.sample_rate_hz, &mut raw_ring_buffer.consumer, rf_multicast_buffer_clone);
    }).join()?;

    let acquisition_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        do_acquisition.run(acquisition_multicast_buffer_clone, sdr_dev.sample_rate_hz, tx_acq, rx_trk);
    }).join()?;

    let trk_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    let trx_manager = TrackingManager::new(15, rx_acq, tx_trk);
    thread::spawn(move || {
        do_tracking(trk_multicast_buffer_clone);
    }).join()?;

    Ok(())
}