use ringbuf::HeapCons;
use crate::rf::samples_buffer::Sample;


fn rf_thread(consumer: &mut HeapCons<Sample>) {
    loop {
        let mut buf: [Sample; 4096] = Default::default();
        let n_samples = consumer.pop_slice(&mut buf);
        if n_samples == 0 {
            // Empty buffer
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }
        else {

        }
    }
}