use std::collections::HashMap;
use soapysdr::{Args, Device, RxStream, StreamSample};
use serde_json::{json, Value};
use crate::sdr_store::sdr_wrapper::{SdrInfo, SdrConfig, SdrDevice, SdrError};
use crate::utils::hashmap_to_args;

pub struct RtlSdr<D: SdrDevice> {
    pub device: Option<D>,
    pub sdr_info: SdrInfo,
    pub sdr_config: SdrConfig,
}

impl SdrDevice for RtlSdr<Device> {
    // Create a new RTL-SDR device with the given arguments
    // The `args` parameter is a string that contains all the
    // device arguments that are obtained from soapy_sdr::enumerate()

    fn new(args: Args) -> Result<Self, SdrError> {
        let mut args_map = HashMap::new();
        for (key, value) in args.iter() {
            args_map.insert(key.to_string(), value.to_string());
        }

        let new_args = hashmap_to_args(args_map)
            .map_err(|e| SdrError::OtherError(e.to_string()))?;
        let dev = SdrDevice::new(args)?;
        let info = RtlSdr::map_args_to_info(new_args)?;
        Ok(Self {
            device: Some(dev),
            sdr_info: info,
            sdr_config: SdrConfig::default(),
        })
    }
}

impl RtlSdr<Device> {

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

    fn start_rx_stream(&mut self, chnls: &[usize], time_ns: Option<i64>) -> Result<(), SdrError> {
        // Implementation for starting the RX stream
        if let Ok(mut rx_stream) = self.device.as_mut().unwrap().rx_stream::<u16>(chnls) {
            rx_stream.activate(time_ns).map_err(|e| SdrError::StreamError(e.to_string()))?;
        }
        Ok(())
    }

    fn stop_rx_stream(&mut self, chnls: &[usize], time_ns: Option<i64>) -> Result<(), SdrError> {
        // Implementation for stopping the RX stream
        if let Ok(mut rx_stream) = self.device.as_mut().unwrap().rx_stream::<u16>(chnls) {
            rx_stream.deactivate(time_ns).map_err(|e| SdrError::StreamError(e.to_string()))?;
        }
        Ok(())
    }

    fn read_samples(&mut self, buf: &mut [i16]) -> Result<usize, SdrError> {
        // Implementation for reading samples into the buffer
        Ok(buf.len())
    }

    fn transmit_samples(&self, buf: &mut [i16]) -> Result<(), SdrError> {
        // Implementation for transmitting samples
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