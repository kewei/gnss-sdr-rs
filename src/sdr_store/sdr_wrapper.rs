use std::collections::HashMap;
use std::fmt::{Formatter, Display, Result as FmtResult};
use std::error::Error;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use soapysdr::{Args, Device, StreamSample};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, EnumIter)]
pub enum DriverName {
    RtlSdr,
    BladeRf,
    HackRf,
    LimeSdr,
    PlutoSdr,
    AirSpy,
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct SdrInfo {
    pub long_args: Option<String>,
    pub manufacturer: Option<String>,
    pub tuner: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub driver: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub product: Option<String>,
}


#[derive(Debug, Clone, Default)]
pub struct SdrConfig {
    pub center_frequency: u64, // Center frequency in Hz
    pub sample_rate: u32, // Sample rate in Hz
    pub gain: f32, // Gain in dB
    pub frequency_correction: Option<u32>, // Frequency correction in ppm
    pub bandwidth: Option<u32>, // Bandwidth in Hz
    pub antenna: Vec<String>, // Antenna
    pub gain_mode: Option<String>, // Gain mode (e.g., 'manual', 'agc')
    pub pps_enabled: bool, // PPS (Pulse Per Second) enabled
    pub extra_config: Option<HashMap<String, String>>, // Additional configuration options
}

pub trait SdrDevice {
    fn new(args: Args) -> Result<Self, SdrError>
    where
        Self: Sized;

    fn map_args_to_info(args: Args) -> Result<SdrInfo, SdrError>
    where 
        Self: Sized {
        let long_args = args.to_string();
        let mut info  = SdrInfo::default();
        info.long_args = Some(long_args);
        info.tuner = args.get("tuner").map(|s| s.to_string());
        info.manufacturer = args.get("manufacturer").map(|s| s.to_string());
        info.model = args.get("model").map(|s| s.to_string());
        info.serial_number = args.get("serial").map(|s| s.to_string());
        info.driver = args.get("driver").map(|s| s.to_string());
        info.label = args.get("label").map(|s| s.to_string());
        info.product = args.get("product").map(|s| s.to_string());

        Ok(info)
    }

    fn start_rx_stream(&mut self) -> Result<(), SdrError> {
        Ok(())
    }
    fn start_tx_stream(&mut self) -> Result<(), SdrError> {
        Ok(())
    }
    fn stop_rx_stream(&mut self) -> Result<(), SdrError> {
        Ok(())
    }
    fn stop_tx_stream(&mut self) -> Result<(), SdrError> {
        Ok(())
    }
    fn read_samples<T: StreamSample>(&mut self, buf: &mut [T]) -> Result<usize, SdrError> {
        Ok(buf.len())
    }
    fn transmit_samples<T: StreamSample>(&self, buf: &mut [T]) -> Result<(), SdrError> {
        Ok(())
    }
}

impl SdrDevice for Device {
    fn new(args: Args) -> Result<Self, SdrError> {
        Device::new(args).map_err(|e| SdrError::DeviceError(e.to_string()))
    }
}

#[derive(Debug)]
pub enum SdrError {
    DeviceNotFound(String),
    DeviceError(String),
    ConfigError(String),
    StreamError(String),
    SampleReadError(String),
    TransmitError(String),
    OtherError(String),
}

impl Display for SdrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            SdrError::DeviceNotFound(msg) => write!(f, "Device not found: {}", msg),
            SdrError::DeviceError(msg) => write!(f, "Device error: {}", msg),
            SdrError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            SdrError::StreamError(msg) => write!(f, "Stream error: {}", msg),
            SdrError::SampleReadError(msg) => write!(f, "Sample read error: {}", msg),
            SdrError::TransmitError(msg) => write!(f, "Transmit error: {}", msg),
            SdrError::OtherError(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl Error for SdrError {}