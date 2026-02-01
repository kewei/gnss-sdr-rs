use std::collections::HashMap;
use soapysdr::{Args, Device};
use serde_json::Value;
use rustfft::num_complex::Complex64;
use crate::sdr_store::sdr_wrapper::{SdrInfo, SdrConfig, SdrDeviceWrapper, SdrError};
use crate::utils::hashmap_to_args;
use crate::sdr_store::rtl_sdr::RtlSdr;

pub struct MockDevice {
    pub device: Option<Device>,
    pub sdr_info: SdrInfo,
    pub sdr_config: SdrConfig,
}

impl SdrDeviceWrapper for MockDevice {
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
        Ok(())
    }

    fn read_samples(&mut self, buf: &mut [&mut [Complex64]], timeout_us: i64) -> Result<usize, SdrError> {
        Ok(buf.len())
    }

    fn transmit_samples(&self, buf: &mut [&mut [Complex64]]) -> Result<(), SdrError> {
        Ok(())
    }
}

impl MockDevice {
    fn new(args: Args) -> Result<Self, SdrError> {
        let dev = Device::new(Args::from("")).map_err(|e| SdrError::DeviceError(e.to_string()))?;
        let info = Self::map_args_to_info(args)?;
        Ok(Self {
            device: Some(dev),
            sdr_info: info,
            sdr_config: SdrConfig::default(),
        })
    }
}

impl RtlSdr<MockDevice> {
    pub fn new(args: Args) -> Result<Self, SdrError> {
        let dev = MockDevice::new(Args::from("")).map_err(|e| SdrError::DeviceError(e.to_string()))?;
        let info = MockDevice::map_args_to_info(args)?;
        Ok(Self {
            device: Some(dev),
            sdr_info: info,
            sdr_config: SdrConfig::default(),
        })
    }
}