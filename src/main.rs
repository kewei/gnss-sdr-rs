#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables)]

use std::ffi::{c_void, CString};
use std::io::Error;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, u8};
mod acquisition;
use acquisition::do_acquisition;
mod tracking;
use tracking::do_track;
mod decoding;
use decoding::nav_decoding;
mod gps_ca_prn;
mod gps_constants;
mod utilities;
use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferRead, RingBufferWrite};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

fn main() -> Result<(), Error> {
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

    let sampling_rate: f32 = 2.046e6;
    let frequency: u32 = 1574.42e6 as u32;
    let freq_IF: f32 = 0.0;
    let mut gain = 0;
    let ppm_error = 0;
    //const default_buf_len: usize = 262144;
    const default_buf_len: usize = 4096 * 4;
    #[allow(unused_variables)]
    let min_buf_len = 256;
    #[allow(unused_variables)]
    let max_buf_len = 4194304;
    const buff_len: usize = default_buf_len * size_of::<u8>();
    let mut buf_vec = [0u8; buff_len];
    let buf: *mut [u8] = &mut buf_vec;
    let mut n_read = 0;
    let mut bytes_read: u32 = 0; // 0 means infinite
    let mut dev = ptr::null_mut();

    unsafe {
        dev = ptr::null_mut() as *mut rtlsdr_dev;
        let r = rtlsdr_open(&mut dev, dev_index as u32);
        if r < 0 {
            panic!("Failed to open rtlsdr device at {}", dev_index);
        }

        if dev.is_null() {
            panic!("Failed to open rtlsdr device at {}", dev_index);
        } else {
            verbose_set_frequency(dev, frequency);
            verbose_set_sample_rate(dev, sampling_rate as u32);
        }

        if gain == 0 {
            /* Enable automatic gain */
            verbose_auto_gain(dev);
        } else {
            /* Enable manual gain */
            gain = nearest_gain(dev, gain);
            verbose_gain_set(dev, gain);
        }

        verbose_ppm_set(dev, ppm_error);
        verbose_reset_buffer(dev); // Reset endpoint before we start reading from it (mandatory)
    }

    let mut ring_buffer: AllocRingBuffer<u8> =
        AllocRingBuffer::with_capacity(max_buf_len * size_of::<u8>());
    let num_ca_code_samples = (sampling_rate
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let acq_len = num_ca_code_samples * 2; // Complex values
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    while !term.load(Ordering::Relaxed) {
        let r: i32;

        unsafe {
            r = rtlsdr_read_sync(dev, buf as *mut c_void, buff_len as i32, &mut n_read);
        }

        if r < 0 {
            println!("WARNING: sync read failed.");
            break;
        } else {
            if (bytes_read > 0) && ((bytes_read as i32) < n_read) {
                n_read = bytes_read as i32;
                break;
            }
            if n_read < buff_len as i32 {
                println!("WARNING: short read! Exit!");
            }

            if bytes_read > 0 {
                bytes_read -= n_read as u32;
            }
            ring_buffer.extend(buf_vec.into_iter());
            //let buf_vec_f32: Vec<f32> = buf_vec.to_vec().iter().map(|x| f32::from(*x)).collect();
            //utilities::plot_psd(&buf_vec_f32, sampling_rate as u32);
        }
        if ring_buffer.is_full() {
            println!("Samples are processed slower than expected! So the data is not consistent anymore.");
            break;
        }

        // Could be Async process?
        let mut samples_input: Vec<i16> = Vec::new();
        for _ in 0..acq_len {
            if let Some(item) = ring_buffer.dequeue() {
                samples_input.push(item as i16);
            } else {
                print!("The value in the buffer is None.");
                break;
            }
        }
        if let Ok(acq_results) = do_acquisition(samples_input, sampling_rate, freq_IF) {
            if let Ok(tracking_result) =
                do_track(samples_input, acq_results, sampling_rate, freq_IF)
            {
                if let Ok(pos_result) = nav_decoding(tracking_result) {
                    break;
                } else {
                    todo!(); // do tracking again with new data
                }
            } else {
                todo!(); // do tracking again with new data
            };
        } else {
            continue;
        };
    }

    unsafe {
        rtlsdr_close(dev);
        libc::free(dev as *mut c_void);
    }
    Ok(())
}
