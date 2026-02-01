use std::fmt::{Display, Formatter};
use serde::Deserialize;
use crate::sdr_store::sdr_wrapper::SdrConfig;

pub static APP_CONFIG_FILE: &str = "config/app_config.toml";

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub device: String,
    pub sdr: SdrConfig,
    pub rf: RfConfig,
    pub pvt: PvtConfig,
    pub output: OutputConfig,
}

#[derive(Deserialize, Debug)]
pub struct RfConfig {
    pub output_sample_rate_hz: u32,
    pub enable_agc: bool,
}

#[derive(Deserialize, Debug)]
pub struct PvtConfig {
    pub enable: bool,
}

#[derive(Deserialize, Debug)]
pub struct OutputConfig {
    pub file_type: String,
}

#[derive(Debug)]
pub struct AppConfigError(String);
impl Display for AppConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "AppConfigError: {}", self.0)
    }
}

impl std::error::Error for AppConfigError {}

impl AppConfig {
    pub fn from_toml_file(file_path: &str) -> Result<Self, AppConfigError> {
        let config_str = std::fs::read_to_string(file_path).map_err(|e| AppConfigError(format!("Failed to read config file: {}", e)))?;
        let config: AppConfig = toml::from_str(config_str.as_str()).map_err(|e| AppConfigError(format!("Failed to parse toml file: {}", e)))?;
        Ok(config)
    }
}