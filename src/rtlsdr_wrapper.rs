use std::ffi::{c_uchar, c_uint, c_void, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct rtlsdr_dev_wrapper {
    pub dev: *mut rtlsdr_dev,
}

impl rtlsdr_dev_wrapper {
    pub fn new() -> Self {
        Self {
            dev: ptr::null_mut() as *mut rtlsdr_dev,
        }
    }

    pub fn open(&mut self) {
        let mut dev_name = String::from("00000001");
        dev_name.retain(|c| c.to_digit(32).unwrap() <= 9);
        let dev_name = CString::new(dev_name).expect("CString::new failed.");
        let mut dev_index = 0;

        unsafe {
            dev_index = verbose_device_search(dev_name.into_raw());
            if dev_index == -1 {
                let dev_name = CString::new("0").expect("CString::new failed.");
                dev_index = verbose_device_search(dev_name.into_raw());
            }
        }

        if dev_index < 0 {
            panic!("Did not find supported device.")
        }

        self.dev = ptr::null_mut() as *mut rtlsdr_dev;
        let r = unsafe { rtlsdr_open(&mut self.dev, dev_index as u32) };
        if r < 0 {
            panic!("Failed to open rtlsdr device at {}", dev_index);
        }

        if self.dev.is_null() {
            panic!("Failed to open rtlsdr device at {}", dev_index);
        }
    }

    pub fn rtlsdr_config(
        &mut self,
        frequency: u32,
        sampling_rate: u32,
        mut gain: i32,
        ppm_error: i32,
    ) {
        unsafe {
            verbose_set_frequency(self.dev, frequency);
            verbose_set_sample_rate(self.dev, sampling_rate as u32);
        }
        if gain == 0 {
            /* Enable automatic gain */
            unsafe {
                verbose_auto_gain(self.dev);
            }
        } else {
            /* Enable manual gain */
            gain = unsafe { nearest_gain(self.dev, gain) };
            unsafe {
                verbose_gain_set(self.dev, gain);
            }
        }

        unsafe {
            verbose_ppm_set(self.dev, ppm_error);
            verbose_reset_buffer(self.dev);
        }
    }

    pub fn rtlsdr_read_async_wrapper(
        &mut self,
        num_buff: u32,
        buff_size: u32,
        stop_signal: Arc<AtomicBool>,
    ) {
        //let dev = ptr::null_mut() as *mut rtlsdr_dev_t;
        while stop_signal.load(Ordering::SeqCst) {
            let ctx = ptr::null_mut();
            unsafe {
                rtlsdr_read_async(
                    self.dev,
                    Some(rust_callback_wrapper),
                    ctx,
                    num_buff,
                    buff_size,
                );
            }
        }
        unsafe { rtlsdr_cancel_async(self.dev) };
        self.rtlsdr_close_wrapper();
    }

    pub fn rtlsdr_close_wrapper(&mut self) {
        unsafe {
            rtlsdr_close(self.dev);
        }
    }
}

unsafe impl Send for rtlsdr_dev_wrapper {}

extern "C" {
    fn rust_callback_wrapper(buff: *mut c_uchar, buff_len: c_uint, ctx: *mut c_void);
}
