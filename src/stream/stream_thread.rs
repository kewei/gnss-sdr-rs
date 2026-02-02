fn stream_thread(consumer: &mut SamplesConsumer) {
    loop {
        let mut bu: [Sample; 4096] = Default::default();
        let n_samples = consumer.pop_slice(&mut bu);
        if n_samples == 0 {
            // Empty buffer
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }
        else {

        }
    }
}