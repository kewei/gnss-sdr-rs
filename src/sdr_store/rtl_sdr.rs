use std::collections::HashMap;
use soapysdr::{Device, Args};
use serde_json::{json, Value};
use crate::sdr_store::sdr_wrapper::{SdrInfo, SdrConfig, SdrDevice, SdrError};
use crate::utils::hashmap_to_args;

pub struct RtlSdr {
    pub device: Option<Device>,
    pub sdr_info: SdrInfo,
    pub sdr_config: SdrConfig,
}

impl RtlSdr {
    // Create a new RTL-SDR device with the given arguments
    // The `args` parameter is a string that contains all the
    // device arguments that are obtained from soapy_sdr::enumerate()
    fn new(&mut self, args: Args) -> Result<Self, SdrError> {
        let mut args_map = HashMap::new();
        for (key, value) in args.iter() {
            args_map.insert(key.to_string(), value.to_string());
        }
        let dev = Device::new(args).map_err(|e| SdrError::DeviceError(e.to_string()))?;

        let new_args = hashmap_to_args(args_map)
            .map_err(|e| SdrError::OtherError(e.to_string()))?;
        let long_args = new_args.to_string();
        let mut info: SdrInfo = Default::default();
        let config: SdrConfig = Default::default();
        info.long_args = Some(long_args);
        info.tuner = new_args.get("tuner").map(|s| s.to_string());
        info.manufacturer = new_args.get("manufacturer").map(|s| s.to_string());
        info.model = new_args.get("model").map(|s| s.to_string());
        info.serial_number = new_args.get("serial").map(|s| s.to_string());
        info.driver = new_args.get("driver").map(|s| s.to_string());
        info.label = new_args.get("label").map(|s| s.to_string());
        info.description = new_args.get("description").map(|s| s.to_string());
        info.version = new_args.get("version").map(|s| s.to_string());
        info.product_name = new_args.get("product_name").map(|s| s.to_string());

        Ok(Self {
            device: Some(dev),
            sdr_info: info,
            sdr_config: config,
        })
    }
}

impl SdrDevice for RtlSdr {

    fn config(&mut self, config: Value) -> Result<(), String> {
        if let Some(c_freq) = config.get("center_frequency").and_then(Value::as_u64) {
            self.sdr_config.center_frequency = c_freq;
        }
        if let Some(s_rate) = config.get("sample_rate").and_then(Value::as_u64) {
            self.sdr_config.sample_rate = s_rate as u32;
        }
        if let Some(gain) = config.get("gain").and_then(Value::as_f64) {
            self.sdr_config.gain = gain as f32;
        }
        if let Some(freq_corr) = config.get("frequency_correction").and_then(Value::as_u64) {
            self.sdr_config.frequency_correction = Some(freq_corr as u32);
        }
        if let Some(bandwidth) = config.get("bandwidth").and_then(Value::as_u64) {
            self.sdr_config.bandwidth = Some(bandwidth as u32);
        }
        if let Some(antenna) = config.get("antenna").and_then(Value::as_array) {
            self.sdr_config.antenna = antenna.iter().filter_map(Value::as_str).map(String::from).collect();
        }
        if let Some(gain_mode) = config.get("gain_mode").and_then(Value::as_str) {
            self.sdr_config.gain_mode = Some(gain_mode.to_string());
        }
        else {
            self.sdr_config.gain_mode = Some("manual".to_string());
        }
        if let Some(pps_enabled) = config.get("pps_enabled").and_then(Value::as_bool) {
            self.sdr_config.pps_enabled = pps_enabled;
        }
        Ok(())
    }

    fn start_stream(&mut self) -> Result<(), String> {
        // Implementation for starting the stream
        Ok(())
    }

    fn stop_stream(&mut self) -> Result<(), String> {
        // Implementation for stopping the stream
        Ok(())
    }

    fn read_samples(&mut self, buf: &mut [i16]) -> Result<usize, String> {
        // Implementation for reading samples into the buffer
        Ok(buf.len())
    }

    fn transmit_samples(&self, buf: &mut [i16]) -> Result<(), String> {
        // Implementation for transmitting samples
        Ok(())
    }
}