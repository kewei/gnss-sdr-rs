use crate::config::app_config::AppConfig;
use crate::rf::frontend::DigitalFrontend;
use crate::rf::samples_buffer::{Sample, SampleComplex, SampleReal, create_samples_ring_buffer};
use num_complex::Complex32;
use ringbuf::HeapCons;

static BLOCK_SIZE: usize = 2048;

fn rf_thread(
    rf_config: &RfConfig,
    input_sample_rate: &f32,
    consumer: &mut HeapCons<Sample>,
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
        if consumer.len() < BLOCK_SIZE {
            // Not enought samples, wait a bit
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        let n_samples = consumer.pop_slice(&mut block);
        if n_samples == 0 {
            // Empty buffer
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        } else {
            if n_samples < BLOCK_SIZE {
                consumer.pop_slice(&mut block[n_samples..]); // Hit the end of the buffer, fill it again
                let mut block_planar = prepare_block(&mut block, BLOCK_SIZE); // size: 2 * BLOCK_SIZE
                frontend.process_block(&mut block_planar);
                let mut written = 0;
                while written < BLOCK_SIZE * 2 {
                    let n = rf_prod.push_slice(&block_planar[written..BLOCK_SIZE * 2]);
                    written += n;
                    if n == 0 {
                        // Output buffer is full, wait a bit
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            }
        }
    }
}

/// It maps the complex samples to a planar format, where the real and imaginary parts are stored interleaved
/// It is safe because the input data is in contigous memory
fn prepare_block(data: &mut [Complex32], len_data: usize) -> (&mut [f32]) {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut f32, len_data * 2) }
}
