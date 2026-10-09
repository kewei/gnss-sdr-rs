use crate::rf::samples_buffer::SampleComplex;
use num_complex::Complex32;
use ringbuf::HeapProd;
use ringbuf::traits::Producer;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read};
use std::time::{Duration, Instant};

const CHUNK_SAMPLES: usize = 8192;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SampleLayout {
    Real,
    Complex,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl DataType {
    fn byte_width(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Endianness {
    Little,
    Big,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FileInputConfig {
    pub path: String,
    pub data_layout: SampleLayout,
    pub data_type: DataType,
    pub endianness: Endianness,
}

pub struct FileInputReader {
    file: File,
    path: String,
    layout: SampleLayout,
    data_type: DataType,
    endianness: Endianness,
    remaining_samples: u64,
    sample_rate_hz: f32,
}

impl FileInputReader {
    pub fn open(config: FileInputConfig, sample_rate_hz: f32) -> io::Result<Self> {
        if !sample_rate_hz.is_finite() || sample_rate_hz <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File input sample rate must be finite and greater than zero",
            ));
        }

        let file = File::open(&config.path)?;
        let bytes_per_sample = config.data_type.byte_width()
            * if config.data_layout == SampleLayout::Complex {
                2
            } else {
                1
            };
        let file_len = file.metadata()?.len();
        if file_len % bytes_per_sample as u64 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "File '{}' length ({file_len} bytes) is not a whole number of configured samples ({bytes_per_sample} bytes each)",
                    config.path
                ),
            ));
        }


        Ok(Self {
            file,
            path: config.path,
            layout: config.data_layout,
            data_type: config.data_type,
            endianness: config.endianness,
            remaining_samples: file_len / bytes_per_sample as u64,
            sample_rate_hz,
        })
    }

    pub fn read_to(&mut self, producer: &mut HeapProd<SampleComplex>) -> io::Result<()> {
        let components_per_sample = if self.layout == SampleLayout::Complex {
            2
        } else {
            1
        };
        let bytes_per_component = self.data_type.byte_width();
        let bytes_per_sample = bytes_per_component * components_per_sample;
        let mut bytes = vec![0; CHUNK_SAMPLES * bytes_per_sample];
        let mut samples = vec![Complex32::new(0.0, 0.0); CHUNK_SAMPLES];
        let playback_start = Instant::now();
        let mut samples_read = 0u64;

        while self.remaining_samples > 0 {
            let sample_count = self.remaining_samples.min(CHUNK_SAMPLES as u64) as usize;
            let byte_count = sample_count * bytes_per_sample;
            self.file
                .read_exact(&mut bytes[..byte_count])
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("Failed reading file input '{}': {error}", self.path),
                    )
                })?;

            for (index, sample) in samples[..sample_count].iter_mut().enumerate() {
                let component_offset = index * bytes_per_sample;
                sample.re = decode_component(
                    &bytes[component_offset..component_offset + bytes_per_component],
                    self.data_type,
                    self.endianness,
                );
                sample.im = if components_per_sample == 2 {
                    decode_component(
                        &bytes[component_offset + bytes_per_component
                            ..component_offset + bytes_per_sample],
                        self.data_type,
                        self.endianness,
                    )
                } else {
                    0.0
                };
            }

            let mut pushed = 0;
            while pushed < sample_count {
                let count = producer.push_slice(&samples[pushed..sample_count]);
                pushed += count;
                if count == 0 {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }

            self.remaining_samples -= sample_count as u64;
            samples_read += sample_count as u64;

            let target_elapsed =
                Duration::from_micros(samples_read as u64 / self.sample_rate_hz as u64);
            if let Some(wait) = target_elapsed.checked_sub(playback_start.elapsed()) {
                std::thread::sleep(wait);
            }
        }

        Ok(())
    }
}

fn decode_component(bytes: &[u8], data_type: DataType, endianness: Endianness) -> f32 {
    let little_endian = matches!(endianness, Endianness::Little);
    match data_type {
        DataType::I8 => (bytes[0] as i8) as f32,
        DataType::U8 => bytes[0] as f32 - 128.0,
        DataType::I16 => {
            let value = if little_endian {
                i16::from_le_bytes([bytes[0], bytes[1]])
            } else {
                i16::from_be_bytes([bytes[0], bytes[1]])
            };
            value as f32
        }
        DataType::U16 => {
            let value = if little_endian {
                u16::from_le_bytes([bytes[0], bytes[1]])
            } else {
                u16::from_be_bytes([bytes[0], bytes[1]])
            };
            value as f32 - 32768.0
        }
        DataType::I32 => {
            let array = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if little_endian {
                i32::from_le_bytes(array) as f32
            } else {
                i32::from_be_bytes(array) as f32
            }
        }
        DataType::U32 => {
            let array = [bytes[0], bytes[1], bytes[2], bytes[3]];
            let value = if little_endian {
                u32::from_le_bytes(array)
            } else {
                u32::from_be_bytes(array)
            };
            (value as f64 - 2_147_483_648.0) as f32
        }
        DataType::I64 => {
            let array = [
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ];
            if little_endian {
                i64::from_le_bytes(array) as f32
            } else {
                i64::from_be_bytes(array) as f32
            }
        }
        DataType::U64 => {
            let array = [
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ];
            let value = if little_endian {
                u64::from_le_bytes(array)
            } else {
                u64::from_be_bytes(array)
            };
            (value as f64 - 9_223_372_036_854_775_808.0) as f32
        }
        DataType::F32 => {
            let array = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if little_endian {
                f32::from_le_bytes(array)
            } else {
                f32::from_be_bytes(array)
            }
        }
        DataType::F64 => {
            let array = [
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ];
            if little_endian {
                f64::from_le_bytes(array) as f32
            } else {
                f64::from_be_bytes(array) as f32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rf::samples_buffer::create_samples_ring_buffer;
    use ringbuf::traits::Consumer;
    use std::time::SystemTime;

    #[test]
    fn decodes_signed_and_unsigned_integer_samples() {
        assert_eq!(
            decode_component(&[0xff], DataType::I8, Endianness::Little),
            -1.0
        );
        assert_eq!(
            decode_component(&[0x00], DataType::U8, Endianness::Big),
            -128.0
        );
        assert_eq!(
            decode_component(&[0x12, 0x34], DataType::I16, Endianness::Big),
            4660.0
        );
        assert_eq!(
            decode_component(&[0x80, 0x00], DataType::U16, Endianness::Big),
            0.0
        );
        assert_eq!(
            decode_component(
                &(-4i32).to_le_bytes(),
                DataType::I32,
                Endianness::Little
            ),
            -4.0
        );
        assert_eq!(
            decode_component(
                &[0x80, 0x00, 0x00, 0x00],
                DataType::U32,
                Endianness::Big
            ),
            0.0
        );
        assert_eq!(
            decode_component(&(-8i64).to_be_bytes(), DataType::I64, Endianness::Big),
            -8.0
        );
        assert_eq!(
            decode_component(
                &9_223_372_036_854_775_808u64.to_be_bytes(),
                DataType::U64,
                Endianness::Big
            ),
            0.0
        );
    }

    #[test]
    fn decodes_floating_point_samples_with_selected_endianness() {
        assert_eq!(
            decode_component(&1.5f32.to_be_bytes(), DataType::F32, Endianness::Big),
            1.5
        );
        assert_eq!(
            decode_component(
                &(-2.25f64).to_le_bytes(),
                DataType::F64,
                Endianness::Little
            ),
            -2.25
        );
    }

    #[test]
    fn reads_complex_f32_file_samples_into_ring_buffer() {
        let path = std::env::temp_dir().join(format!(
            "gnss-sdr-rs-file-input-{}-{}.bin",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bytes = [1.0f32, -0.5, 0.25, 0.75]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        std::fs::write(&path, bytes).unwrap();

        let config = FileInputConfig {
            path: path.to_string_lossy().into_owned(),
            data_layout: SampleLayout::Complex,
            data_type: DataType::F32,
            endianness: Endianness::Little,
        };
        let mut reader = FileInputReader::open(config, 1_000_000_000.0).unwrap();
        let mut ring_buffer = create_samples_ring_buffer(16);
        reader.read_to(&mut ring_buffer.producer).unwrap();

        let mut actual = [Complex32::new(0.0, 0.0); 2];
        assert_eq!(ring_buffer.consumer.pop_slice(&mut actual), 2);
        assert_eq!(
            actual,
            [Complex32::new(1.0, -0.5), Complex32::new(0.25, 0.75)]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_a_trailing_partial_sample() {
        let path = std::env::temp_dir().join(format!(
            "gnss-sdr-rs-partial-sample-{}-{}.bin",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, [1, 2, 3]).unwrap();
        let config = FileInputConfig {
            path: path.to_string_lossy().into_owned(),
            data_layout: SampleLayout::Complex,
            data_type: DataType::I8,
            endianness: Endianness::Little,
        };

        let error = match FileInputReader::open(config, 1_000_000.0) {
            Ok(_) => panic!("expected a partial complex sample to be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn deserializes_file_input_format_from_toml() {
        let config: FileInputConfig = toml::from_str(
            r#"
                path = "capture.bin"
                data_layout = "real"
                data_type = "i16"
                endianness = "big"
            "#,
        )
        .unwrap();

        assert_eq!(config.data_layout, SampleLayout::Real);
        assert!(matches!(config.data_type, DataType::I16));
        assert!(matches!(config.endianness, Endianness::Big));
    }
}
