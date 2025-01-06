use std::{
    borrow::Cow,
    io::{self, Write},
    mem::size_of,
    num::NonZeroU16,
};

use cpal::{I24, I48, U24, U48};
use itertools::Itertools;
use num::traits::ToBytes;
use thiserror::Error;

use crate::Sample;

#[derive(Debug, Clone)]
pub struct WaveFile<'a> {
    pub format: Format,
    pub channels: NonZeroU16,
    pub sample_rate: u32,
    pub bytes_per_sample: u16,
    pub data: Cow<'a, [u8]>,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum Format {
    PulseCodeModulation = 1,
    FloatingPoint = 3,
}

#[derive(Error, Debug)]
pub enum WaveFileWriteError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("data too long")]
    DataTooLong,
}

pub trait SampleType {
    const SAMPLE_TYPE: Format;
}

macro_rules! impl_sample_type {
    ($($ty:ty: $format:ident)+) => {
        $(
            impl SampleType for $ty {
                const SAMPLE_TYPE: Format = Format::$format;
            }
        )+
    };
}

impl_sample_type! {
    f32: FloatingPoint
    f64: FloatingPoint
    i8: PulseCodeModulation
    i16: PulseCodeModulation
    i32: PulseCodeModulation
    i64: PulseCodeModulation
    u8: PulseCodeModulation
    u16: PulseCodeModulation
    u32: PulseCodeModulation
    u64: PulseCodeModulation
    I24: PulseCodeModulation
    I48: PulseCodeModulation
    U24: PulseCodeModulation
    U48: PulseCodeModulation
}

impl<'a> WaveFile<'a> {
    /// Create a new [`WaveFile`] from an iterable of samples and a sample rate, or [`None`] if the number of channels does not fit in a [`NonZeroU16`], because it was zero or more than [`u16::MAX`].
    /// # Panics
    /// Panics if the size of the sample type is too large to fit in a [`u16`], because there were more than [`u16::MAX`] channels.
    pub fn from_samples<T: cpal::Sample + SampleType + ToBytes<Bytes = [u8; N]>, const N: usize, S: Into<Sample<T, N>>>(samples: impl IntoIterator<Item = S>, sample_rate: u32) -> Option<Self> {
        let format = T::SAMPLE_TYPE;
        let channels = NonZeroU16::new(u16::try_from(N).ok()?)?;
        let bytes_per_sample = u16::try_from(size_of::<T>()).expect("size of sample type is too large");
        let data = samples
            .into_iter()
            .map_into()
            .flat_map(|Sample(channels)| channels.map(|sample| sample.to_le_bytes()))
            .flatten()
            .collect_vec()
            .into();
        Some(Self {
            format,
            channels,
            sample_rate,
            bytes_per_sample,
            data,
        })
    }

    #[must_use]
    pub fn from_raw_data(data: &'a [u8], format: Format, channels: NonZeroU16, sample_rate: u32, bytes_per_sample: u16) -> Self {
        Self {
            format,
            channels,
            sample_rate,
            bytes_per_sample,
            data: data.into(),
        }
    }

    /// Write the [`WaveFile`] to a writer.
    /// # Errors
    /// Returns an [`WaveFileWriteError::Io`] if writing to the writer fails (from calls to [`Write::write_all`]), or [`WaveFileWriteError::DataTooLong`] if the data was longer than [`u32::MAX`]
    /// bytes.
    pub fn write(&self, writer: &mut impl Write) -> Result<(), WaveFileWriteError> {
        const BYTES_AFTER_FILE_LENGTH_AND_BEFORE_SAMPLE_DATA: usize = 4 + 4 + 4 + 2 + 2 + 4 + 4 + 2 + 2 + 4 + 4;
        writer.write_all(b"RIFF")?;
        writer.write_all(
            &u32::try_from(BYTES_AFTER_FILE_LENGTH_AND_BEFORE_SAMPLE_DATA + self.data.len())
                .map_err(|_| WaveFileWriteError::DataTooLong)?
                .to_le_bytes(),
        )?;
        writer.write_all(b"WAVEfmt ")?;
        writer.write_all(&[16])?;
        writer.write_all(&(self.format as u16).to_le_bytes())?;
        writer.write_all(&self.channels.get().to_le_bytes())?;
        writer.write_all(&self.sample_rate.to_le_bytes())?;
        writer.write_all(&(self.sample_rate * u32::from(self.channels.get()) * u32::from(self.bytes_per_sample)).to_le_bytes())?;
        writer.write_all(&(self.bytes_per_sample * 8).to_le_bytes())?;
        writer.write_all(b"data")?;
        writer.write_all(&u32::try_from(self.data.len()).map_err(|_| WaveFileWriteError::DataTooLong)?.to_le_bytes())?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}
