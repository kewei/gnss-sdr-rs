use num::complex::Complex32;
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

#[derive(Debug, Clone)]
pub struct MulticastRingBuffError;

impl fmt::Display for MulticastRingBuffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MulticastRingBuffer lock error in notifier.")
    }
}

impl Error for MulticastRingBuffError {}

impl<T> From<PoisonError<T>> for MulticastRingBuffError {
    fn from(_: PoisonError<T>) -> Self {
        MulticastRingBuffError
    }
}

// TODO: We can consider using a more zero-copy approach in the future:
// Option 1: when reading samples, we can return a reference to the buffer 
// instead of copying data to a separate buffer, returning two slices if 
// the data is wrapped around. But we need to be careful about the lifetime
// of the reference and ensure that it is not used after the buffer is 
// overwritten by new samples.
// Option 2: Virtual Memory Double Mapping, OS system calls (mmap on Linux/macOS,
// VirtualAlloc on Windows). If your buffer size is N bytes, Virtual Address 
// 0 to N points to your physical memory. Virtual Address N to 2N points
// to the exact same physical memory.
pub struct MulticastRingBuffer {
    pub buffer: Vec<UnsafeCell<Complex32>>,
    buf_size: usize,
    mask: usize,           // For fast modulo: index & mask
    pub head: AtomicUsize, // Written by DFE
    pub notifier: Mutex<bool>,
    pub condvar: Condvar,
}

impl MulticastRingBuffer {
    pub fn new(buf_size: usize) -> Self {
        assert!(
            buf_size.is_power_of_two(),
            "Buffer size must be a power of two"
        );
        Self {
            buffer: (0..buf_size)
                .map(|_| UnsafeCell::new(Complex32::new(0.0, 0.0)))
                .collect(),
            buf_size: buf_size,
            mask: buf_size - 1,
            head: AtomicUsize::new(0),
            notifier: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    /// It is safe because the input samples are in contigous memory and the buffer is also a large
    /// contiguous memory. But, copying data is still costly, we can consider using a more zero-copy
    /// approach in the future
    pub fn write_samples(&self, samples: &[Complex32]) -> Result<(), MulticastRingBuffError> {
        let current_head = self.head.load(Ordering::Relaxed);
        let start = current_head & self.mask;
        let n = samples.len();

        unsafe {
            let ptr = self.buffer.as_ptr() as *mut Complex32;
            let dest = ptr.add(start);

            // Can we use a more zero-copy approach to write samples to the buffer?
            // For example, we can use a separate buffer to store the samples and then swap
            // the buffer pointer with the ring buffer pointer. This way, we can avoid copying
            // data and improve performance.

            if start + n <= self.buf_size {
                std::ptr::copy_nonoverlapping(samples.as_ptr(), dest, n);
            } else {
                let first_part = self.buf_size - start;
                std::ptr::copy_nonoverlapping(samples.as_ptr(), dest, first_part);
                std::ptr::copy_nonoverlapping(
                    samples.as_ptr().add(first_part),
                    ptr,
                    n - first_part,
                );
            }
        }

        self.head.store(current_head + n, Ordering::Release);

        // Wake up Tracking after writing new samples
        let mut guard = self.notifier.lock()?;
        *guard = true;
        self.condvar.notify_all();

        Ok(())
    }

    pub fn get_head(&self) -> usize {
        self.head.load(Ordering::Acquire)
    }

    pub fn copy_to_slice(&self, start: usize, dest: &mut [Complex32]) {
        let n = dest.len();
        let physical_start = start & self.mask;

        unsafe {
            let ptr = self.buffer.as_ptr() as *const Complex32;
            let src = ptr.add(physical_start);
            if physical_start + n <= self.buf_size {
                // dest.copy_from_slice(&self.buffer[physical_start..physical_start + n]);
                std::ptr::copy_nonoverlapping(src, dest.as_mut_ptr(), n);
            } else {
                let first_part = self.buf_size - physical_start;
                // dest[..first_part].copy_from_slice(&self.buffer[physical_start..]);
                // dest[first_part..].copy_from_slice(&self.buffer[..n - first_part]);
                std::ptr::copy_nonoverlapping(src, dest.as_mut_ptr(), first_part);
                std::ptr::copy_nonoverlapping(
                    ptr,
                    dest.as_mut_ptr().add(first_part),
                    n - first_part,
                );
            }
        }
    }
}

unsafe impl Sync for MulticastRingBuffer {}

#[cfg(test)]
mod tests {
    use std::cell::UnsafeCell;

    use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
    use num_complex::Complex;

    fn as_slice(r_cells: &[UnsafeCell<Complex<f32>>]) -> &[Complex<f32>] {
        unsafe {
            std::slice::from_raw_parts(r_cells.as_ptr() as *const Complex<f32>, r_cells.len())
        }
    }

    #[test]
    fn test_multicast_ring_buffer() {
        let ring_buf = MulticastRingBuffer::new(1024);
        let samples: Vec<Complex<f32>> = (0..500).map(|i| Complex::new(i as f32, 0.0)).collect();
        let _ = ring_buf.write_samples(&samples);
        assert_eq!(ring_buf.get_head(), 500);

        let more_samples = (500..1030)
            .map(|i| Complex::new(i as f32, 0.0))
            .collect::<Vec<_>>();
        let _ = ring_buf.write_samples(&more_samples);
        assert_eq!(ring_buf.get_head(), 1030);
        assert_eq!(
            as_slice(&ring_buf.buffer[1020..1024]),
            (1020..1024)
                .map(|i| Complex::new(i as f32, 0.0))
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(
            as_slice(&ring_buf.buffer[0..6]),
            (1024..1030)
                .map(|i| Complex::new(i as f32, 0.0))
                .collect::<Vec<_>>()
                .as_slice()
        );

        let mut dest = vec![Complex::new(0.0, 0.0); 10];
        let _ = ring_buf.copy_to_slice(1020, &mut dest);
        assert_eq!(
            dest,
            (1020..1030)
                .map(|i| Complex::new(i as f32, 0.0))
                .collect::<Vec<_>>()[..]
        );

        let more_samples = (1030..1050)
            .map(|i| Complex::new(i as f32, 0.0))
            .collect::<Vec<_>>();
        let _ = ring_buf.write_samples(&more_samples);
        assert_eq!(ring_buf.get_head(), 1050);
        assert_eq!(
            as_slice(&ring_buf.buffer[1020..1024]),
            (1020..1024)
                .map(|i| Complex::new(i as f32, 0.0))
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(
            as_slice(&ring_buf.buffer[0..6]),
            (1024..1030)
                .map(|i| Complex::new(i as f32, 0.0))
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(
            as_slice(&ring_buf.buffer[6..16]),
            (1030..1040)
                .map(|i| Complex::new(i as f32, 0.0))
                .collect::<Vec<_>>()
                .as_slice()
        );
    }
}
