use crate::config::app_config::AppConfig;
use crate::rf::frontend::DigitalFrontend;
use crate::rf::samples_buffer::{Sample, create_samples_ring_buffer};
use ringbuf::HeapCons;

static BLOCK_SIZE: usize = 2048;

fn rf_thread(
    rf_config: &RfConfig,
    input_sample_rate: &f32,
    consumer: &mut HeapCons<Sample>,
    rf_prod: &mut HeapProd<Sample>,
) {
    let mut buf = create_samples_ring_buffer(2 * BLOCK_SIZE);
    let mut block = [Sample::new(0.0, 0.0); BLOCK_SIZE];
    let mut frontend = DigitalFrontend::new(
        rf_config.freq_if_hz.unwrap_or(0.0),
        *input_sample_rate,
        rf_config.output_sample_rate_hz,
    );
    loop {
        let n_samples = consumer.pop_slice(&mut buf.producer);
        if n_samples == 0 {
            // Empty buffer
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        } else {
            if buf.consumer.len() >= BLOCK_SIZE {
                buf.consumer.pop_slice(&mut block);
                let mut block_re = block.iter().map(|s| s.re).collect::<Vec<f32>>();
                let mut block_im = block.iter().map(|s| s.im).collect::<Vec<f32>>();
                frontend.process_block(&mut block_re, &mut block_im);

                rf_prod.push_slice(&block);
            }
        }
    }
}
