use std::collections::HashMap;
use std::thread;
use std::time;
use signal_hook::low_level::channel;
use soapysdr::{Args, Device, RxStream, StreamSample, Direction};
use serde_json::{json, Value};
use rustfft::{num_complex::Complex32};
use crate::sdr_store::sdr_wrapper::{SdrInfo, SdrConfig, SdrDeviceWrapper, SdrError};
use crate::utils::hashmap_to_args;


pub struct RtlSdr<D: SdrDeviceWrapper> {
    pub device: Option<D>,
    pub sdr_info: SdrInfo,
    pub sdr_config: SdrConfig,
}

impl SdrDeviceWrapper for RtlSdr<Device> {
    fn device(&self) -> Option<&Device> {
        self.device.as_ref()
    }

    fn device_mut(&mut self) -> Option<&mut Device> {
        self.device.as_mut()
    }

    fn get_config(&self) -> SdrConfig {
        self.sdr_config.clone()
    }

    fn config(&mut self, config: Value) -> Result<(), String> {
        if let Some(c_freq) = config.get("center_frequency").and_then(Value::as_f64) {
            self.device.as_ref().unwrap().set_frequency(Direction::Rx, 0, c_freq, Args::from(""))
                .map_err(|e| format!("Failed to set center frequency: {}", e))?;
            self.sdr_config.center_frequency_hz = c_freq;
            thread::sleep(time::Duration::from_millis(100));
        }

        if let Some(s_rate) = config.get("sample_rate").and_then(Value::as_f64) {
            self.device.as_ref().unwrap().set_sample_rate(Direction::Rx, 0, s_rate)
                .map_err(|e| format!("Failed to set sample rate: {}", e))?;
            self.sdr_config.sample_rate_hz = s_rate as f32;
            thread::sleep(time::Duration::from_millis(100));
        }

        if let Some(gain) = config.get("gain").and_then(Value::as_f64) {
            self.device.as_ref().unwrap().set_gain(Direction::Rx, 0, gain)
                .map_err(|e| format!("Failed to set gain: {}", e))?;
            self.sdr_config.gain_db = gain as f32;
            thread::sleep(time::Duration::from_millis(100));
        }

        self.sdr_config.frequency_correction = Some(config.get("frequency_correction").and_then(Value::as_f64).unwrap_or_default() as f32);
        thread::sleep(time::Duration::from_millis(100));
        //  {
        //      = freq_corr as f32;
        //     thread::sleep(time::Duration::from_millis(100));
        // }
        // else {
        //     self.sdr_config.frequency_correction = 0.0;
        //     thread::sleep(time::Duration::from_millis(100));
        // }

        if let Some(bandwidth) = config.get("bandwidth").and_then(Value::as_f64) {
            self.device.as_ref().unwrap().set_bandwidth(Direction::Rx, 0, bandwidth)
                .map_err(|e| format!("Failed to set bandwidth: {}", e))?;
            self.sdr_config.bandwidth_hz = bandwidth as f32;
            thread::sleep(time::Duration::from_millis(100));
        }
        else {
            Err("Bandwidth must be specified for SDR device".to_string())?;
        }        

        if let Ok(antennas) = self.device.as_ref().unwrap().antennas(Direction::Rx, 0) {
            if !antennas.is_empty() {
                self.device.as_ref().unwrap().set_antenna(Direction::Rx, 0, antennas[0].clone())
                    .map_err(|e| format!("Failed to set antenna: {}", e))?;
                self.sdr_config.antennas = Some(vec![antennas[0].to_string()]);
                thread::sleep(time::Duration::from_millis(100));
            }
        }
        
        if let Some(gain_mode) = config.get("gain_mode").and_then(Value::as_str) {
            self.device.as_ref().unwrap().set_gain_mode(Direction::Rx, 0, true)
                .map_err(|e| format!("Failed to set automatic gain mode: {}", e))?;
            self.sdr_config.gain_mode = Some(gain_mode.to_string());
            thread::sleep(time::Duration::from_millis(100));
        }
        else {
            self.device.as_ref().unwrap().set_gain_mode(Direction::Rx, 0, false)
                .map_err(|e| format!("Failed to set manual gain mode: {}", e))?;
            self.sdr_config.gain_mode = Some("agc".to_string());
            thread::sleep(time::Duration::from_millis(100));
        }

        if let Some(pps_enabled) = config.get("pps_enabled").and_then(Value::as_bool) {
            self.sdr_config.pps_enabled = Some(pps_enabled);
            thread::sleep(time::Duration::from_millis(100));
        }

        Ok(())
    }

    fn read_samples(&mut self, buf: &mut [&mut [Complex32]], timeout_us: i64) -> Result<usize, SdrError> {
        // Implementation for reading samples into the buffer, rtl-sdr only has one RX channel
        let mut rx_stream = self.device.as_mut().unwrap().rx_stream::<Complex32>(&[0]).map_err(|e| SdrError::StreamError(e.to_string()))?; 
        rx_stream.activate(None).map_err(|e| SdrError::StreamError(e.to_string()))?;

        let n_samples = rx_stream.read(buf, timeout_us).map_err(|e| SdrError::StreamError(e.to_string()));
        
        n_samples
    }

    fn transmit_samples(&self, buf: &mut [&mut [Complex32]]) -> Result<(), SdrError> {
        // Implementation for transmitting samples
        Ok(())
    }
}

impl RtlSdr<Device> {

    // Create a new RTL-SDR device with the given arguments
    // The `args` parameter is a string that contains all the
    // device arguments that are obtained from soapy_sdr::enumerate()
    pub fn new(args: Args) -> Result<Self, SdrError> {
        let mut args_map = HashMap::new();
        for (key, value) in args.iter() {
            args_map.insert(key.to_string(), value.to_string());
        }

        let new_args = hashmap_to_args(args_map)
            .map_err(|e| SdrError::OtherError(e.to_string()))?;
        let dev = Device::new(args).map_err(|e| SdrError::DeviceError(e.to_string()))?;
        let info = RtlSdr::map_args_to_info(new_args)?;
        Ok(Self {
            device: Some(dev),
            sdr_info: info,
            sdr_config: SdrConfig::default(),
        })
    } 

    fn start_rx_stream(&mut self, chnls: &[usize], time_ns: Option<i64>) -> Result<(), SdrError> {
        // Implementation for starting the RX stream
        if self.device.is_none() {
            return Err(SdrError::DeviceError("Device not initialized".to_string()));
        }
        Ok(())
    }

    fn stop_rx_stream(&mut self, chnls: &[usize], time_ns: Option<i64>) -> Result<(), SdrError> {
        // Implementation for stopping the RX stream
        Ok(())
    }

    
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdr_mock::device_mock::MockDevice;

    #[test]
    fn test_rtl_sdr_driver() {
        let args = Args::new();
        let rtl_sdr = RtlSdr::<MockDevice>::new(args).expect("Failed to mock a RTL-SDR device");
        assert!(rtl_sdr.sdr_info.long_args.is_none());
        assert!(rtl_sdr.sdr_info.serial_number.is_none());
        assert!(rtl_sdr.device.is_some());
    }

    #[test]
    fn test_rtl_sdr_args() {
        let args_str = "driver=rtlsdr, label=Generic RTL2832U OEM :: 00000001, manufacturer=Realtek, product=RTL2838UHIDIR, serial=00000001, tuner=Rafael Micro R820T";
        let args = Args::from(args_str);
        let rtl_sdr = RtlSdr::<MockDevice>::new(Args::from(args_str)).expect("Failed to mock a RTL-SDR device");
        assert!(rtl_sdr.sdr_info.serial_number == Some("00000001".to_string()));
        assert!(rtl_sdr.sdr_info.tuner == Some("Rafael Micro R820T".to_string()));
        assert!(rtl_sdr.sdr_info.manufacturer == Some("Realtek".to_string()));
        assert!(rtl_sdr.sdr_info.product == Some("RTL2838UHIDIR".to_string()));
        assert!(rtl_sdr.device.is_some());
    }

}