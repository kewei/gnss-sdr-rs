use std::collections::HashMap;
use soapysdr::{Args, Device};
use serde_json::Value;
use crate::sdr_store::sdr_wrapper::{SdrInfo, SdrConfig, SdrDevice, SdrError};
use crate::utils::hashmap_to_args;
use crate::sdr_store::rtl_sdr::RtlSdr;

pub struct MockDevice {
    pub device: Option<Device>,
    pub sdr_info: SdrInfo,
    pub sdr_config: SdrConfig,
}

impl SdrDevice for MockDevice {
    fn new(args: Args) -> Result<Self, SdrError> {
        let dev = Device::new("").map_err(|e| SdrError::DeviceError(e.to_string()))?;
        let info = MockDevice::map_args_to_info(args)?;
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