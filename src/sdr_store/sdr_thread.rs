use crate::rf::samples_buffer::SampleComplex;
use crate::sdr_store::sdr_wrapper::SdrConfig;
use crate::sdr_store::sdr_wrapper::SdrDeviceWrapper;
use crate::sdr_store::sdr_wrapper::SdrError;
use crate::sdr_store::sdr_wrapper::start_device_with_name;
use num_complex::Complex32;
use ringbuf::HeapProd;
use ringbuf::traits::Producer;
use serde_json::json;
// use soapysdr::Direction::Rx;

const ACTIVATION_TIME: i64 = 10000000; // 10ms
const DEV_TIMEOUT: i64 = 100000; // 100ms

// Start RX stream with channel 0
pub fn sdr_thread(
    device: String,
    sdr: SdrConfig,
    prod: &mut HeapProd<SampleComplex>,
) -> Result<(), SdrError> {
    let mut sdr_dev = start_device_with_name(device)?;
    sdr_dev.config(json!(&sdr))?;
    sdr_dev.start_rx_stream(Some(ACTIVATION_TIME))?;
    let mtu: usize = sdr_dev
        .get_rx_stream_mute()
        .ok_or(SdrError::StreamError(
            "Rx stream not initialized".to_string(),
        ))?
        .mtu()
        .map_err(|e| SdrError::StreamError(format!("Failed to get RX stream MTU: {}", e)))?;
    // let num_channels = dev.num_channels(Rx)?;  // Not really matter for GNSS
    let mut buf = vec![Complex32::new(0.0, 0.0); mtu];
    loop {
        let n_samples = sdr_dev.read_samples(&mut [&mut buf[..]], DEV_TIMEOUT)?;
        if n_samples > 0 {
            let mut started = 0;
            while started < n_samples {
                let pushed = prod.push_slice(&buf[started..n_samples]);
                started += pushed;

                if pushed == 0 {
                    // Buffer is full, wait a bit
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
    }
}
