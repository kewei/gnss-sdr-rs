use async_ffi::async_ffi;
use libc::c_void;
use once_cell::sync::Lazy;
use rayon::iter::plumbing::Producer;
use ringbuf::{HeapConsumer, HeapProducer, HeapRb};
use std::ffi::{c_uchar, c_uint};
use std::mem::size_of;
use std::slice;
use std::sync::{Arc, RwLock};
use tokio::task;

pub const RTL_BUFF_NUM: usize = 32;
pub const BUFFER_SIZE: usize = 16384;
pub const APP_BUFFER_NUM: usize = 6000;

pub struct AppBuffer {
    pub buff_cnt: Arc<RwLock<usize>>,
    pub buff_producer: HeapProducer<u8>,
    pub buff_consumer: HeapConsumer<u8>,
}

impl AppBuffer {
    pub fn new() -> Self {
        let ring_buff = HeapRb::<u8>::try_new(APP_BUFFER_NUM * BUFFER_SIZE * 2 * size_of::<u8>())
            .expect("Error occurs while creating AppBuffer");
        let (mut producer, mut consumer) = ring_buff.split();
        Self {
            buff_cnt: Arc::new(RwLock::new(0)),
            buff_producer: producer,
            buff_consumer: consumer,
        }
    }
}

// Global variable for application buffer
pub static mut APPBUFF: Lazy<AppBuffer> = Lazy::new(|| AppBuffer::new());

pub async fn callback_read_buffer(buff: Arc<*const c_uchar>, buff_len: c_uint) {
    let data_ptr = Arc::as_ptr(&buff);
    let _data = unsafe { *data_ptr };
    let data_slice: &[u8] = unsafe { slice::from_raw_parts(_data, 2 * buff_len as usize) };
    let cnt_clone = unsafe { Arc::clone(&(APPBUFF.buff_cnt)) };
    let mut cnt_val = cnt_clone
        .write()
        .expect("Error in locking when incrementing buff_cnt of AppBuffer");
    let added_data = unsafe { APPBUFF.buff_producer.push_slice(data_slice) };
    assert_eq!(added_data, 2 * buff_len as usize);
    *cnt_val = (*cnt_val + 1) % APP_BUFFER_NUM;
}

#[no_mangle]
#[async_ffi(?Send)]
pub async extern "C" fn c_callback_read_buffer(buff: *const c_uchar, buff_len: c_uint) {
    let data_ptr = Arc::new(buff);
    callback_read_buffer(data_ptr, buff_len).await;
}

pub fn get_current_buffer(buffer_location: usize, n_samples: usize) -> Vec<i16> {
    let cnt_clone = unsafe { Arc::clone(&(APPBUFF.buff_cnt)) };
    let cnt_val = cnt_clone
        .read()
        .expect("Error in locking when reading buff_cnt of AppBuffer");
    todo!();
}
