use crossbeam_channel;
use gnss_sdr_rs::acquisition::do_acquisition;
use gnss_sdr_rs::acquisition::do_acquisition::AcquisitionResult;
use gnss_sdr_rs::config::app_config::{APP_CONFIG_FILE, AppConfig};
use gnss_sdr_rs::input::app_input::app_input;
use gnss_sdr_rs::rf::rf_thread::rf_thread;
use gnss_sdr_rs::tracking::do_tracking;
use gnss_sdr_rs::tracking::do_tracking::TrackingMessage;
use gnss_sdr_rs::utilities::multicast_ring_buffer::MulticastRingBuffer;
use gnss_sdr_rs::data::acquisition_data::AcquisitionData;
use gnss_sdr_rs::visualization::acquisition_gui::AcquisitionGui;
use gnss_sdr_rs::visualization::app_gui::GnssSdrRsGui;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("----------- GNSS-SDR-RS started -------------");

    // Load the application configuration
    let app_config = AppConfig::from_toml_file(APP_CONFIG_FILE)?;
    println!("------- Starting input: {:?}", app_config.device);

    let (mut raw_ring_buffer_consumer, file_input_finished, file_is_complex) = app_input(&app_config)?;

    // We use a large buffer to store the samples. RF thread wrties to it, and the acquisition and tracking threads
    // read from it.
    let multicast_buffer: Arc<MulticastRingBuffer> = Arc::new(MulticastRingBuffer::new(1 << 20)); // 1M Complex32 samples, 8MB
    let (tx_acq, rx_acq) = crossbeam_channel::unbounded::<AcquisitionResult>();
    let (tx_trk, rx_trk) = crossbeam_channel::unbounded::<TrackingMessage>();
    let (tx_acq_gui, rx_acq_gui) = std::sync::mpsc::channel::<AcquisitionData>();

    let sample_rate_hz = app_config.sdr.sample_rate_hz;
    let rf_config = app_config.rf;
    let freq_if_hz = rf_config.freq_if_hz.unwrap_or(0.0);
    let complex_signal = rf_config.complex_signal || file_is_complex;
    let pipeline_finished = Arc::new(AtomicBool::new(false));
    let acquisition_finished = Arc::new(AtomicBool::new(false));

    let rf_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    let rf_input_finished = Arc::clone(&file_input_finished);
    let rf_pipeline_finished = Arc::clone(&pipeline_finished);
    let rf_finished_buffer = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        if let Err(error) = rf_thread(
            &rf_config,
            sample_rate_hz,
            &mut raw_ring_buffer_consumer,
            rf_multicast_buffer_clone,
            rf_input_finished,
            Arc::clone(&rf_pipeline_finished),
        ) {
            eprintln!("RF thread failed: {error}");
            rf_pipeline_finished.store(true, Ordering::Release);
            if let Err(notify_error) = rf_finished_buffer.notify_waiters() {
                eprintln!("Failed to notify pipeline workers at shutdown: {notify_error}");
            }
        }
    });

    let acquisition_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    let acquisition_pipeline_finished = Arc::clone(&pipeline_finished);
    let acquisition_finished_flag = Arc::clone(&acquisition_finished);
    let acquisition_finished_buffer = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        if let Err(error) = do_acquisition::run(
            acquisition_multicast_buffer_clone,
            sample_rate_hz,
            freq_if_hz,
            complex_signal,
            tx_acq,
            rx_trk,
            tx_acq_gui,
            acquisition_pipeline_finished,
        ) {
            eprintln!("Acquisition thread failed: {error}");
        }
        acquisition_finished_flag.store(true, Ordering::Release);
        if let Err(error) = acquisition_finished_buffer.notify_waiters() {
            eprintln!("Failed to notify tracking thread at acquisition shutdown: {error}");
        }
    });

    let trk_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    let tracking_pipeline_finished = Arc::clone(&pipeline_finished);
    let tracking_acquisition_finished = Arc::clone(&acquisition_finished);
    thread::spawn(move || {
        if let Err(error) = do_tracking::run(
            trk_multicast_buffer_clone,
            rx_acq,
            tx_trk,
            sample_rate_hz,
            tracking_pipeline_finished,
            tracking_acquisition_finished,
        ) {
            eprintln!("Tracking thread failed: {error}");
        }
    });

    let acq_gui = AcquisitionGui::new(rx_acq_gui);
    let app = GnssSdrRsGui::new(acq_gui);
    eframe::run_native("GNSS-SDR-RS", eframe::NativeOptions::default(), Box::new(|_cc| Ok(Box::new(app))))?;

    Ok(())
}
