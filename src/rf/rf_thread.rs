use crate::config::app_config::RfConfig;
use crate::rf::frontend::DigitalFrontend;
use crate::rf::samples_buffer::SampleComplex;
use crate::utilities::multicast_ring_buffer::{
    MulticastRingBuffError, MulticastRingBuffer,
};
use num_complex::Complex32;
use ringbuf::traits::{Consumer, Observer};
use ringbuf::HeapCons;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

static BLOCK_SIZE: usize = 2048;

pub fn rf_thread(
    rf_config: &RfConfig,
    input_sample_rate: f32,
    sdr_consumer: &mut HeapCons<SampleComplex>,
    shared_ring_buffer: Arc<MulticastRingBuffer>,
    input_finished: Arc<AtomicBool>,
    pipeline_finished: Arc<AtomicBool>,
) -> Result<(), MulticastRingBuffError> {
    let mut block = [SampleComplex::new(0.0, 0.0); BLOCK_SIZE];
    let mut frontend = DigitalFrontend::new(
        rf_config.freq_if_hz.unwrap_or(0.0),
        input_sample_rate,
        rf_config.output_sample_rate_hz,
    );

    loop {
        let available = sdr_consumer.occupied_len();
        if available < BLOCK_SIZE && !input_finished.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let n_samples = sdr_consumer.pop_slice(&mut block[..available.min(BLOCK_SIZE)]);
        if n_samples == 0 {
            if input_finished.load(Ordering::Acquire) {
                break;
            }
            continue;
        }

        block[n_samples..].fill(SampleComplex::new(0.0, 0.0));
        let mut block_planar = prepare_block(&mut block, BLOCK_SIZE);
        frontend.process_block(&mut block_planar);
        let block_complex = post_process_block(&mut block_planar, BLOCK_SIZE * 2);
        shared_ring_buffer.write_samples(&block_complex[..n_samples])?;
    }

    pipeline_finished.store(true, Ordering::Release);
    shared_ring_buffer.notify_waiters()?;
    Ok(())
}

fn prepare_block(data: &mut [Complex32], len_data: usize) -> &mut [f32] {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut f32, len_data * 2) }
}

fn post_process_block(data: &mut [f32], len_data: usize) -> &mut [Complex32] {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut Complex32, len_data / 2) }
}
