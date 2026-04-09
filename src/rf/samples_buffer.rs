use ringbuf::{traits::*, HeapRb, HeapProd, HeapCons};
use num::complex::Complex;

pub type SampleComplex = Complex<f32>;
pub type SampleReal = f32;
pub static BUFFER_SIZE: usize = 327680;


pub struct SamplesRingBuffer<T> {
    pub producer: HeapProd<T>,
    pub consumer: HeapCons<T>,
}

pub fn create_samples_ring_buffer<T>(size: usize) -> SamplesRingBuffer<T> {
    let rb = HeapRb::<T>::new(size);
    let (mut producer, mut consumer) = rb.split();
    SamplesRingBuffer { producer, consumer }
}