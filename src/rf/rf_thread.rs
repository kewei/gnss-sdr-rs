use crate::config::app_config::AppConfig;
use crate::rf::frontend::DigitalFrontend;
use crate::rf::samples_buffer::{Sample, SampleComplex, SampleReal, create_samples_ring_buffer};
use crate::utilities::multicast_ring_buffer::{ArcMulticastRingBuffer, MulticastRingBuffer};
use num_complex::Complex32;
use ringbuf::HeapCons;

static BLOCK_SIZE: usize = 2048;

fn rf_thread(
    rf_config: &RfConfig,
    input_sample_rate: &f32,
    sdr_consumer: &mut HeapCons<Sample>,
    shared_ring_buffer: Arc<MulticastRingBuffer>,
    rf_prod: &mut HeapProd<Sample>,
) {
    // let mut buf = create_samples_ring_buffer::<SampleComplex>(8 * BLOCK_SIZE);
    let mut block = [SampleComplex::new(0.0, 0.0); BLOCK_SIZE];
    let mut frontend = DigitalFrontend::new(
        rf_config.freq_if_hz.unwrap_or(0.0),
        *input_sample_rate,
        rf_config.output_sample_rate_hz,
    );
    loop {
        if sdr_consumer.len() < BLOCK_SIZE {
            // Not enought samples, wait a bit
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let n_samples = sdr_consumer.pop_slice(&mut block);
        if n_samples == 0 {
            // Empty buffer
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        } else {
            if n_samples < BLOCK_SIZE {  // Hit the end of the buffer
                sdr_consumer.pop_slice(&mut block[n_samples..]); // Then, fill it again

                !todo!("Is it costly to do prepare_block and post_process_block? Can we improve it?");
                
                let mut block_planar = prepare_block(&mut block, BLOCK_SIZE); // size: 2 * BLOCK_SIZE
                frontend.process_block(&mut block_planar);
                let block_complex = post_process_block(&mut block_planar, BLOCK_SIZE * 2);
                shared_ring_buffer.write_samples(&block_complex);
                // let mut written = 0;
                // while written < BLOCK_SIZE * 2 {
                //     let n = rf_prod.push_slice(&block_planar[written..BLOCK_SIZE * 2]);
                //     written += n;
                //     if n == 0 {
                //         // Output buffer is full, wait a bit
                //         std::thread::sleep(std::time::Duration::from_millis(5));
                //     }
                // }
            }
        }
    }
}

/// It maps the complex samples to a planar format, where the real and imaginary parts are stored interleaved
/// It is safe because the input data is in contigous memory
fn prepare_block(data: &mut [Complex32], len_data: usize) -> (&mut [f32]) {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut f32, len_data * 2) }
}

/// It maps the planar samples to a complex format, where the real and imaginary parts are stored interleaved
/// It is safe because the input data is in contigous memory
fn post_process_block(data: &mut [f32], len_data: usize) -> (&mut [Complex32]) {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut Complex32, len_data / 2) }
}