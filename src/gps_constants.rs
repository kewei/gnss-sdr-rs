#![allow(dead_code)]

pub const GPS_L1_FREQ_HZ: f32 = 1.57542e9;
pub const GPS_L1_CA_CODE_RATE_CHIPS_PER_S: f32 = 1.023e6; // chips/s
pub const GPS_L1_CA_CODE_LENGTH_CHIPS: f32 = 1023.0; // chips
pub const GPS_L1_CA_CODE_PERIOD_S: f32 = 1.0e-3; // seconds
pub const GPS_L1_CA_CHIP_PERIOD_S: f32 = GPS_L1_CA_CODE_PERIOD_S / GPS_L1_CA_CODE_LENGTH_CHIPS; // seconds
pub const GPS_L1_CA_CODE_PERIOD_MS: u16 = 1; // ms
pub const GPS_L1_CA_BIT_PERIOD_MS: u16 = 20; // ms

// Navigation message
pub const GPS_CA_PREAMBLE: Vec<i8> = vec![1, -1, -1, -1, 1, -1, 1, 1];
pub const GPS_CA_PREAMBLE_DURATION_S: f32 = 0.160;
pub const GPS_CA_PREAMBLE_LENGTH_BITS: u16 = 8;
pub const GPS_CA_PREAMBLE_LENGTH_SYMBOLS: i16 = 160;
pub const GPS_CA_PREAMBLE_DURATION_MS: u16 = 160;
pub const GPS_CA_TELEMETRY_RATE_BITS_PER_S: u16 = 50;
pub const GPS_CA_TELEMETRY_SYMBOLS_PER_BIT: u16 = 20;
pub const GPS_CA_TELEMETRY_RATE_SYMBOLS_PER_S: u16 = 1000;

pub const GPS_WORD_BITS: u16 = 30;
pub const GPS_WORD_LENGTH_BYTES: u16 = 4;
pub const GPS_SUBFRAME_LENGTH_BYTES: u16 = 40;
pub const GPS_SUBFRAME_BITS: u16 = 300;
pub const GPS_SUBFRAME_MS: u16 = 6000;
