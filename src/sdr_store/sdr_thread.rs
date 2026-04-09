use crate::rf::samples_buffer::SampleReal;
use crate::sdr_store::sdr_wrapper::SdrDeviceWrapper;
use crate::sdr_store::sdr_wrapper::SdrError;
use num_complex::Complex32;
use ringbuf::HeapProd;
use ringbuf::traits::Producer;
// use soapysdr::Direction::Rx;

pub fn sdr_thread(
    dev: &mut impl SdrDeviceWrapper,
    prod: &mut HeapProd<SampleReal>,
) -> Result<(), SdrError> {
    let mtu: usize = dev
        .get_rx_stream_mute()
        .ok_or(SdrError::StreamError(
            "Rx stream not initialized".to_string(),
        ))?
        .mtu()
        .map_err(|e| SdrError::StreamError(format!("Failed to get RX stream MTU: {}", e)))?;
    // let num_channels = dev.num_channels(Rx)?;  // Not really matter for GNSS
    let mut buf = vec![Complex32::new(0.0, 0.0); mtu];
    let mut buffers = [&mut buf[..]];
    loop {
        let n_samples = dev.read_samples(&mut buffers, 100000)?;
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
