use num::complex::Complex;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct MulticastRingBuffer {
    buffer: Vec<Complex<f32>>,
    mask: usize,       // For fast modulo: index & mask
    head: AtomicUsize, // Written by DFE
}

impl MulticastRingBuffer {
    pub fn new(size: usize) -> Self {
        assert!(size.is_power_of_two(), "Buffer size must be a power of two");
        Self {
            buffer: Vec::with_capacity(size),
            mask: size - 1,
            head: AtomicUsize::new(0),
        }
    }

    /// It is safe because the input samples are in contigous memory and the buffer is also a large 
    /// contiguous memory. But, copying data is still costly, we can consider using a more zero-copy 
    /// approach in the future
    pub fn write_samples(&self, samples: &[Complexf32]) {
        let start = self.head.load(Ordering::Relaxed) & self.mask;
        let n = samples.len();

        unsafe {
            let ptr = self.buffer.as_ptr() as *mut Complex<f32>;
            let dest = ptr.add(start);
            !todo!(
                "Can we use a more zero-copy approach to write samples to the buffer? 
                For example, we can use a separate buffer to store the samples and then swap 
                the buffer pointer with the ring buffer pointer. This way, we can avoid copying 
                data and improve performance."
            );
            if start + n <= self.buffer.len() {
                std::ptr::copy_nonoverlapping(samples.as_ptr(), dest, n);
            } else {
                let first_part = self.buffer.len() - start;
                std::ptr::copy_nonoverlapping(samples.as_ptr(), dest, first_part);
                std::ptr::copy_nonoverlapping(
                    samples.as_ptr().add(first_part),
                    ptr,
                    n - first_part,
                );
            }
        }

        self.head.store(start + n, Ordering::Release);
    }

    pub fn get_head(&self) -> usize {
        self.head.load(Ordering::Acquire)
    }

    pub fn get_sample_at(&self, index: usize) -> Complex<f32> {
        self.buffer[index & self.mask]
    }
}
