use async_ffi::async_ffi;
use itertools::Itertools;
use libc::c_void;
use once_cell::sync::Lazy;
use rayon::iter::plumbing::Producer;
use slice_ring_buf::{SliceRB, SliceRbRef};
use std::ffi::{c_uchar, c_uint};
use std::mem::size_of;
use std::slice;
use std::sync::{Arc, RwLock};
use tokio::task;

pub const RTL_BUFF_NUM: usize = 32;
pub const BUFFER_SIZE: usize = 16384;
pub const APP_BUFFER_NUM: usize = 6000;

pub struct AppBuffer {
    pub buff_cnt: usize,
    pub app_buffer: SliceRB<u8>,
}

impl AppBuffer {
    pub fn new() -> Self {
        Self {
            buff_cnt: 0,
            app_buffer: SliceRB::<u8>::from_len(APP_BUFFER_NUM * 2 * BUFFER_SIZE),
        }
    }
}

// Global variable for application buffer
pub static mut APPBUFF: Lazy<Arc<RwLock<AppBuffer>>> =
    Lazy::new(|| Arc::new(RwLock::new(AppBuffer::new())));

#[no_mangle]
pub extern "C" fn callback_read_buffer(buff: Arc<*const c_uchar>, buff_len: c_uint) {
    let app_buffer_clone = unsafe { Arc::clone(&APPBUFF) };
    let mut app_buffer_clone_val = app_buffer_clone
        .write()
        .expect("Error in locking when writing to AppBuffer");

    // Copy data
    let data_ptr = Arc::as_ptr(&buff);
    let _data = unsafe { *data_ptr };
    let data_slice: &[u8] = unsafe { slice::from_raw_parts(_data, 2 * BUFFER_SIZE) };

    let cnt = app_buffer_clone_val.buff_cnt;
    app_buffer_clone_val
        .app_buffer
        .write_latest(data_slice, (cnt * 2 * BUFFER_SIZE) as isize);

    // Increment buff_cnt
    app_buffer_clone_val.buff_cnt = (app_buffer_clone_val.buff_cnt + 1) % APP_BUFFER_NUM;
    println!("buff_cnt: {}", app_buffer_clone_val.buff_cnt);
}

#[no_mangle]
pub extern "C" fn rust_callback_wrapper(buff: *const c_uchar, buff_len: c_uint, ctx: *mut c_void) {
    let data_ptr = Arc::new(buff);
    callback_read_buffer(data_ptr, buff_len);
}

/// Reading samples from the circular buffer
///
pub fn get_current_buffer(buffer_location: usize, n_samples: usize) -> Vec<i16> {
    let buffer_clone = unsafe { Arc::clone(&APPBUFF) };
    let buffer_val = buffer_clone
        .read()
        .expect("Error happens when reading data from AppBuffer");

    let mut out_data = vec![0; n_samples];
    let buf_loc = 2 * buffer_location % (APP_BUFFER_NUM * 2 * BUFFER_SIZE);
    buffer_val
        .app_buffer
        .read_into(&mut out_data, buf_loc as isize);

    out_data.iter().map(|&x| x as i16).collect::<Vec<i16>>()
}
