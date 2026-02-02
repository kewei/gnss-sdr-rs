use crate::sdr_store::sdr_wrapper::SdrDeviceWrapper;
use crate::stream::samples_buffer::Sample;
use ringbuf::HeapProd;
use ringbuf::traits::Producer;

pub fn sdr_thread(dev: &mut impl SdrDeviceWrapper, prod: &mut HeapProd<Sample>) {
    loop {
            let mut buf: [Sample; 4096] = Default::default();
            let mut buffers = [&mut buf[..]];
            let n_samples = dev.read_samples(&mut buffers, 100000)?;
            if n_samples == 0 {
                continue;
            }

            let mut started = 0;
            while started < n_samples {
                let pushed = prod.push_slice(&buf[started..n_samples]);
                started += pushed;

                if pushed == 0 {
                    // Buffer is full, wait a bit
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            } 
            
        }
}