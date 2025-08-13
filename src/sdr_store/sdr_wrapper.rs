use std::collections::HashMap;
use std::fmt::{Formatter, Display, Result as FmtResult};
use std::error::Error;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use soapysdr::Device;

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
    pub product_name: Option<String>,
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
    fn config(&mut self, config: Value) -> Result<(), String>;
    fn start_stream(&mut self) -> Result<(), String>;
    fn stop_stream(&mut self) -> Result<(), String>;
    fn read_samples(&mut self, buf: &mut [i16]) -> Result<usize, String>;
    fn transmit_samples(&self, buf: &mut [i16]) -> Result<(), String>;
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