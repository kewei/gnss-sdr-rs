use ringbuf::{traits::*, HeapRb, HeapProd, HeapCons};
use num::complex::Complex;

pub type Sample = Complex<f32>;
pub static BUFFER_SIZE: usize = 16384;


pub struct SamplesRingBuffer {
    pub producer: HeapProd<Sample>,
    pub consumer: HeapCons<Sample>,
}

pub fn create_samples_ring_buffer(size: usize) -> SamplesRingBuffer {
    let rb = HeapRb::<Sample>::new(size);
    let (mut producer, mut consumer) = rb.split();
    SamplesRingBuffer { producer, consumer }
}