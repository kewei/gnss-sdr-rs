use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use ringbuf::HeapCons;

use crate::config::app_config::AppConfig;
use crate::input::file_input::{FileInputReader, SampleLayout};
use crate::rf::samples_buffer::{BUFFER_SIZE, SampleComplex, create_samples_ring_buffer};
use crate::sdr_store::sdr_thread::sdr_thread;

pub fn app_input(
    app_config: &AppConfig,
) -> Result<(HeapCons<SampleComplex>, Arc<AtomicBool>, bool), Box<dyn Error>> {
    let file_input_finished = Arc::new(AtomicBool::new(false));
    let file_is_complex = app_config
        .file_input
        .as_ref()
        .is_some_and(|config| config.data_layout == SampleLayout::Complex);

    let (mut producer, consumer) = {
        let raw_ring_buffer = create_samples_ring_buffer::<SampleComplex>(BUFFER_SIZE);
        (raw_ring_buffer.producer, raw_ring_buffer.consumer)
    };

    if app_config.device == "file" {
        let file_config = app_config
            .file_input
            .clone()
            .ok_or_else(|| "device = \"file\" requires a [file_input] section in the application config")?;
        let file_reader = FileInputReader::open(file_config, app_config.sdr.sample_rate_hz)?;
        let file_input_finished_clone = Arc::clone(&file_input_finished);
        thread::spawn(move || {
            let mut file_reader = file_reader;
            if let Err(error) = file_reader.read_to(&mut producer) {
                eprintln!("File input failed: {error}");
            }
            file_input_finished_clone.store(true, Ordering::Release);
        });
    } else {
        let file_input_finished_clone = Arc::clone(&file_input_finished);
        let sdr_clone = app_config.sdr.clone();
        let device_name = app_config.device.clone();
        thread::spawn(move || {
            if let Err(error) = sdr_thread(device_name, sdr_clone, &mut producer) {
                eprintln!("SDR thread failed: {error}");
            }
            file_input_finished_clone.store(true, Ordering::Release);
        });
    }

    Ok((consumer, file_input_finished, file_is_complex))
}