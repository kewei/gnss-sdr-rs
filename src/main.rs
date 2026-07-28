use crossbeam_channel;
use gnss_sdr_rs::acquisition::do_acquisition;
use gnss_sdr_rs::acquisition::do_acquisition::AcquisitionResult;
use gnss_sdr_rs::config::app_config::{APP_CONFIG_FILE, AppConfig};
use gnss_sdr_rs::rf::rf_thread::rf_thread;
use gnss_sdr_rs::rf::samples_buffer::{BUFFER_SIZE, SampleComplex, create_samples_ring_buffer};
use gnss_sdr_rs::sdr_store::sdr_thread::sdr_thread;
use gnss_sdr_rs::sdr_store::sdr_wrapper::SdrDeviceWrapper;
use gnss_sdr_rs::sdr_store::sdr_wrapper::start_device_with_name;
use gnss_sdr_rs::tracking::do_tracking;
use gnss_sdr_rs::tracking::do_tracking::TrackingMessage;
use gnss_sdr_rs::utilities::multicast_ring_buffer::MulticastRingBuffer;
use serde_json::json;
use std::sync::Arc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("----------- GNSS-SDR-RS started -------------");

    // Load the application configuration
    let app_config = AppConfig::from_toml_file(APP_CONFIG_FILE)?;
    println!("Starting stream with device: {:?}", app_config.device);

    let mut sdr_dev = start_device_with_name(app_config.device)?;
    sdr_dev.config(json!(&app_config.sdr))?;

    let mut raw_ring_buffer = create_samples_ring_buffer::<SampleComplex>(BUFFER_SIZE);

    // We use a large buffer to store the samples from RF thread, and then the acquisition and tracking threads
    // can read from it. Here only the RF thread will write to the buffer, and the acquisition and tracking threads
    // will read from it, so we don't need to worry about concurrent write and read.
    let multicast_buffer: Arc<MulticastRingBuffer> = Arc::new(MulticastRingBuffer::new(1 << 20)); // 1M Complex32 samples, 8MB
    let (tx_acq, rx_acq) = crossbeam_channel::unbounded::<AcquisitionResult>();
    let (tx_trk, rx_trk) = crossbeam_channel::unbounded::<TrackingMessage>();

    thread::spawn(move || {
        let _ = sdr_thread(&mut sdr_dev, &mut raw_ring_buffer.producer);
    })
    .join()
    .map_err(|e| format!("SDR thread failed: {:?}", e))?;

    let rf_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        rf_thread(
            &app_config.rf,
            app_config.sdr.sample_rate_hz,
            &mut raw_ring_buffer.consumer,
            rf_multicast_buffer_clone,
        );
    })
    .join()
    .map_err(|e| format!("RF thread failed: {:?}", e))?;

    let acquisition_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        let _ = do_acquisition::run(
            acquisition_multicast_buffer_clone,
            app_config.sdr.sample_rate_hz,
            app_config.rf.freq_if_hz.unwrap_or(0.0),
            tx_acq,
            rx_trk,
        );
    })
    .join()
    .map_err(|e| format!("Acquisition thread failed: {:?}", e))?;

    let trk_multicast_buffer_clone = Arc::clone(&multicast_buffer);
    thread::spawn(move || {
        let _ = do_tracking::run(
            trk_multicast_buffer_clone,
            rx_acq,
            tx_trk,
            app_config.sdr.sample_rate_hz,
        );
    })
    .join()
    .map_err(|e| format!("Tracking thread failed: {:?}", e))?;

    Ok(())
}
