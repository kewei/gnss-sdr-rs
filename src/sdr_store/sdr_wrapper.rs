use std::collections::HashMap;
use std::fmt::{Formatter, Display, Result as FmtResult};
use std::error::Error;
use std::hash::Hash;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use signal_hook::low_level::channel;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use soapysdr::{Args, Device, StreamSample, Direction, RxStream, TxStream, Range};
use num_complex::Complex32;
use crate::sdr_store::rtl_sdr::RtlSdr;

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


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SdrConfig {
    pub center_frequency_hz: f64, // Center frequency in Hz
    pub sample_rate_hz: f32, // Sample rate in Hz
    pub gain_db: f32, // Gain in dB
    pub bandwidth_hz: f32, // Bandwidth in Hz
    pub frequency_correction: Option<f32>, // Frequency correction in ppm
    pub antennas: Option<Vec<String>>, // Antennas
    pub gain_mode: Option<String>, // Gain mode (e.g., 'manual', 'agc')
    pub pps_enabled: Option<bool>, // PPS (Pulse Per Second) enabled
    pub extra_config: Option<HashMap<String, String>>, // Additional configuration options
}

pub trait SdrDeviceWrapper: Send + Sync {
    fn device(&self) -> Option<&Device>;

    fn device_mut(&mut self) -> Option<&mut Device>;

    fn get_config(&self) -> SdrConfig;

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

    /// List available antennas for the given direction and channel.
    fn antennas(&self, direction: Direction, channel: usize) -> Result<Vec<String>, SdrError> {
        self.device().unwrap().antennas(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the selected antenna for the given direction and channel.
    fn antenna(&self, direction: Direction, channel: usize) -> Result<String, SdrError> {
        self.device().unwrap().antenna(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Set the selected antenna for the given direction and channel.
    fn set_antenna(&self, direction: Direction, channel: usize, antenna: &str) -> Result<(), SdrError> {
        self.device().unwrap().set_antenna(direction, channel, antenna).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the baseband filter width of the chain in Hz
    fn bandwidth(&self, direction: Direction, channel: usize) -> Result<f64, SdrError> {
        self.device().unwrap().bandwidth(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the range of valid bandwidths for the given direction and channel.
    fn bandwidth_range(&self, direction: Direction, channel: usize) -> Result<Vec<Range>, SdrError> {
        self.device().unwrap().bandwidth_range(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Set the baseband filter width of the chain in Hz
    fn set_bandwidth(&self, direction: Direction, channel: usize, bandwidth: f64) -> Result<(), SdrError> {
        self.device().unwrap().set_bandwidth(direction, channel, bandwidth).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    fn driver_key(&self) -> Result<String, SdrError> {
        self.device().unwrap().driver_key().map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Returns the down-conversion frequency in Hz.
    fn frequency(&self, direction: Direction, channel: usize) -> Result<f64, SdrError> {
        self.device().unwrap().frequency(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the range of valid frequencies for the given direction and channel.
    fn frequency_range(&self, direction: Direction, channel: usize) -> Result<Vec<Range>, SdrError> {
        self.device().unwrap().frequency_range(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Set the down-conversion frequency in Hz.
    fn set_frequency(&self, direction: Direction, channel: usize, frequency: f64) -> Result<(), SdrError> {
        self.device().unwrap().set_frequency(direction, channel, frequency, Args::from("")).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the overall value of the gain elements in a chain in dB.
    fn gain(&self, direction: Direction, channel: usize) -> Result<f64, SdrError> {
        self.device().unwrap().gain(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the range of valid gain values for the given direction and channel.
    fn gain_range(&self, direction: Direction, channel: usize) -> Result<Range, SdrError> {
        self.device().unwrap().gain_range(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Set the overall amplification in a chain.
    fn set_gain(&self, direction: Direction, channel: usize, gain: f64) -> Result<(), SdrError> {
        self.device().unwrap().set_gain(direction, channel, gain).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Returns true if automatic gain control is enabled
    fn gain_mode(&self, direction: Direction, channel: usize) -> Result<bool, SdrError> {
        self.device().unwrap().gain_mode(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Enable or disable automatic gain control.
    fn set_gain_mode(&self, direction: Direction, channel: usize, automatic: bool) -> Result<(), SdrError> {
        self.device().unwrap().set_gain_mode(direction, channel, automatic).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Query a dictionary of available device information.
    /// This dictionary can any number of values like vendor name, product name, revisions, serials…
    /// This information can be displayed to the user to help identify the instantiated device.
    fn hardware_info(&self) -> Result<Args, SdrError> {
        self.device().unwrap().hardware_info().map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// List available tunable elements in the chain.
    /// Elements should be in order RF to baseband.
    fn list_frequencies(&self, direction: Direction, channel: usize) -> Result<Vec<String>, SdrError> {
        let freqs = self.device().unwrap().list_frequencies(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))?;
        Ok(freqs)
    }

    /// Get the number of channels for the given direction.
    fn num_channels(&self, direction: Direction) -> Result<usize, SdrError> {
        self.device().unwrap().num_channels(direction).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    fn rx_stream(&self, channels: &[usize]) -> Result<RxStream<Complex32>, SdrError> {
        self.device().unwrap().rx_stream(channels).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the baseband sample rate of the chain in samples per second.
    fn sample_rate(&self, direction: Direction, channel: usize) -> Result<f64, SdrError> {
        self.device().unwrap().sample_rate(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Get the range of valid sample rates for the given direction and channel.
    fn get_sample_rate_range(&self, direction: Direction, channel: usize) -> Result<Vec<Range>, SdrError> {
        self.device().unwrap().get_sample_rate_range(direction, channel).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Set the baseband sample rate of the chain in samples per second.
    fn set_sample_rate(&self, direction: Direction, channel: usize, sample_rate: f64) -> Result<(), SdrError> {
        self.device().unwrap().set_sample_rate(direction, channel, sample_rate).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    fn tx_stream(&self, channels: &[usize]) -> Result<TxStream<Complex32>, SdrError> {
        self.device().unwrap().tx_stream(channels).map_err(|e| SdrError::OtherError(e.to_string()))
    }

    /// Set configuration options using key/value pairs.
    fn config(&mut self, config: Value) -> Result<(), String>;

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

    /// Reading samples into the buffer
    fn read_samples(&mut self, buf: &mut [&mut [Complex32]], timeout_us: i64) -> Result<usize, SdrError>;

    /// Transmitting samples from the buffer
    fn transmit_samples(&self, buf: &mut [&mut [Complex32]]) -> Result<(), SdrError>;
}

impl SdrDeviceWrapper for Device {
    fn device(&self) -> Option<&Device> {
        Some(self)
    }

    fn device_mut(&mut self) -> Option<&mut Device> {
        Some(self)
    }

    fn get_config(&self) -> SdrConfig {
        SdrConfig::default()
    }

    fn config(&mut self, config: Value) -> Result<(), String> {
        Ok(())
    }

    fn read_samples(&mut self, buf: &mut [&mut [Complex32]], timeout_us: i64) -> Result<usize, SdrError> {
        Ok(buf.len())
    }

    fn transmit_samples(&self, buf: &mut [&mut [Complex32]]) -> Result<(), SdrError> {
        Ok(())
    }
}

// pub fn create_device(sdr: DriverName, args: Args) -> Result<Box<dyn SdrDevice + Send>, SdrError> {
//     match sdr {
//         DriverName::RtlSdr => RtlSdr::<Device>::new(args).map(|dev| Box::new(dev) as Box<dyn SdrDevice + Send>),
//         _ => Err(SdrError::DeviceNotFound(format!("Driver not found: {:?}", sdr))),
//     }
// }

pub fn start_device_with_name(device_name: String, args: Option<Args>) -> Result<impl SdrDeviceWrapper, SdrError> {
    let mut devs_args: Vec<Args> = Vec::new();
    let mut args = Args::new();
    args.set("driver", device_name.clone());
    for dev in soapysdr::enumerate(args).map_err(|e| SdrError::OtherError(e.to_string()))? {
        devs_args.push(dev);
    }
    if devs_args.is_empty() {
        return Err(SdrError::DeviceNotFound(format!("No device found for driver: {}", device_name)));
    }
    else {
        if devs_args.len() > 1 {
            println!("Warning: Multiple devices found for driver: {}. Using the first one.", device_name);
        }
        let first_dev_args = devs_args[0].iter();

        match device_name.as_str() {
            "rtlsdr" => {
                let rtl_sdr = RtlSdr::<Device>::new(first_dev_args.collect())?;
                Ok(rtl_sdr)
            },
            _ => Err(SdrError::DeviceNotFound(format!("Driver not supported: {}", device_name))),
        }
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