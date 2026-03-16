use num_complex::Complex;

pub struct SamplesBlock {
    pub samples: Vec<Complex<f32>>,
    pub sample_rate_hz:u32,
}

pub struct BlockExtractor {
    buf: Vec<Complex<f32>>,
    block_size: usize,
    sample_rate_hz:u32,
}

impl BlockExtractor {
    pub fn new(sample_rate_hz: u32, block_ms: u32) -> Self {
        let block_size = (sample_rate_hz * block_ms) as usize / 1000;
        Self { buf: Vec::with_capacity(2 * block_size), block_size, sample_rate_hz }
    }

    pub fn push_sample(&mut self, sample: Complex<f32>) -> Option<SamplesBlock> {
        self.buf.push(sample);

        if self.buf.len() >= self.block_size {
            let out: Vec<Complex<f32>> = self.buf.drain(..self.block_size).collect();
            Some(SamplesBlock { samples: out, sample_rate_hz: self.sample_rate_hz })
        } else {
            None
        }
    }
}