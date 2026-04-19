use num::complex::Complex;
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
    pub fn write_samples(&self, samples: &[Complex<f32>]) {
        let start = self.head.load(Ordering::Relaxed) & self.mask;
        let n = samples.len();

        unsafe {
            let ptr = self.buffer.as_ptr() as *mut Complex<f32>;
            let dest = ptr.add(start);
            
            todo!(
                "Can we use a more zero-copy approach to write samples to the buffer? 
                For example, we can use a separate buffer to store the samples and then swap 
                the buffer pointer with the ring buffer pointer. This way, we can avoid copying 
                data and improve performance."
            );

            let buffer_len = self.buffer.len();
            
            if start + n <= buffer_len {
                std::ptr::copy_nonoverlapping(samples.as_ptr(), dest, n);
            } else {
                let first_part = buffer_len - start;
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

    pub fn copy_to_slice(&self, start: usize, dest: &mut [Complex<f32>]) {
        let n = dest.len();
        let physical_start = start & self.mask;
        let buffer_len = self.buffer.len();

        if physical_start + n <= buffer_len {
            dest.copy_from_slice(&self.buffer[physical_start..physical_start + n]);
        } else {
            let mid = buffer_len - physical_start;
            dest[..mid].copy_from_slice(&self.buffer[physical_start..]);
            dest[mid..].copy_from_slice(&self.buffer[..n - mid]);
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
    use num::complex::Complex;

    #[test]
    fn test_multicast_ring_buffer() {
        let ring_buf = MulticastRingBuffer::new(1024);
        let samples: Vec<Complex<f32>> = (0..500).map(|i| Complex::new(i as f32, 0.0)).collect();
        ring_buf.write_samples(&samples);
        assert_eq!(ring_buf.get_head(), 500);
        assert_eq!(ring_buf.get_head(), 500);

        let more_samples = (500..1030).map(|i| Complex::new(i as f32, 0.0)).collect::<Vec<_>>();
        ring_buf.write_samples(&more_samples);
        assert_eq!(ring_buf.get_head(), 1030);
        assert_eq!(ring_buf.buffer[1020..1024], (1020..1024).map(|i| Complex::new(i as f32, 0.0)).collect::<Vec<_>>()[..]);
        assert_eq!(ring_buf.buffer[0..6], (1024..1030).map(|i| Complex::new(i as f32, 0.0)).collect::<Vec<_>>()[..]);

        let mut dest = vec![Complex::new(0.0, 0.0); 10];
        ring_buf.copy_to_slice(1020, &mut dest);
        assert_eq!(dest, (1020..1030).map(|i| Complex::new(i as f32, 0.0)).collect::<Vec<_>>()[..]);
    }
}
