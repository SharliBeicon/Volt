use std::{
    borrow::Cow,
    fmt::Debug,
    io::{self, Write},
    mem::size_of,
    num::NonZeroU16,
};

use cpal::{FromSample, Sample, I24, I48, U24, U48};
use itertools::Itertools;
use num::traits::ToBytes;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct WaveFile<'a> {
    pub format: Format,
    pub channels: NonZeroU16,
    pub sample_rate: u32,
    pub bytes_per_sample: u16,
    pub data: Cow<'a, [u8]>,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Error, Debug)]
pub enum FromSamplesError {
    #[error("channels are not all the same length")]
    InequalChannelLength,
    #[error("too many channels")]
    TooManyChannels,
}

pub trait SampleExt<Unsigned = Self>: Sized + Sample + FromSample<f64> + Debug {
    const SAMPLE_FORMAT: Format;
    const BYTES_PER_SAMPLE: u16 = {
        assert!(size_of::<Self>() <= u16::MAX as usize);
        #[allow(clippy::cast_possible_truncation, reason = "we just checked it")]
        {
            size_of::<Self>() as u16
        }
    };

    fn to_wav_sample(self) -> Unsigned;
}

macro_rules! impl_sample_ext {
    (@to_wav_sample) => {
        fn to_wav_sample(self) -> Self {
            self
        }
    };
    (@to_wav_sample $ty:ty) => {
        fn to_wav_sample(self) -> $ty {
            use cpal::{Sample};
            <$ty>::from_sample(self)
        }
    };
    (@signed_impl) => {};
    (@signed_impl $unsigned:ty, $ty:ty, $format:ident) => {
        impl SampleExt for $ty {
            const SAMPLE_FORMAT: Format = Format::$format;
            impl_sample_ext!(@to_wav_sample);
        }
    };
    ($($ty:ty, $format:ident $(,$unsigned:ty)?;)+) => {
        $(
            impl SampleExt $(< $unsigned >)? for $ty {
                const SAMPLE_FORMAT: Format = Format::$format;
                impl_sample_ext!(@to_wav_sample $($unsigned)?);
            }
            impl_sample_ext!(@signed_impl $($unsigned, $ty, $format)?);
        )+
    };
}

impl_sample_ext! {
    f32, FloatingPoint;
    f64, FloatingPoint;
    i8, PulseCodeModulation, u8;
    i16, PulseCodeModulation;
    i32, PulseCodeModulation;
    i64, PulseCodeModulation;
    u8, PulseCodeModulation;
    u16, PulseCodeModulation, i16;
    u32, PulseCodeModulation, i32;
    u64, PulseCodeModulation, i64;
    I24, PulseCodeModulation, U24;
    I48, PulseCodeModulation, U48;
    U24, PulseCodeModulation, I24;
    U48, PulseCodeModulation, I48;
}

impl<'a> WaveFile<'a> {
    /// Create a new [`WaveFile`] from an iterable of samples and a sample rate, or [`None`] if the number of channels does not fit in a [`NonZeroU16`], because it was zero or more than [`u16::MAX`].
    /// # Panics
    /// Panics if the size of the sample type is too large to fit in a [`u16`], because there were more than [`u16::MAX`] channels.
    /// # Errors
    /// If the number of channels does not fit in a [`NonZeroU16`], because it is zero, or more than [`u16::MAX`], return [`FromSamplesError::TooManyChannels`].
    /// If the channels are not all the same length, return [`FromSamplesError::InequalChannelLength`].
    pub fn from_samples<T: SampleExt + ToBytes<Bytes = [u8; BYTES]>, const BYTES: usize, C: IntoIterator<Item = f64>>(
        channels: impl IntoIterator<Item = C>,
        sample_rate: u32,
    ) -> Result<Self, FromSamplesError> {
        let format = T::SAMPLE_FORMAT;
        let mut channels = channels
            .into_iter()
            .map(|channel| channel.into_iter().map(|sample: f64| T::from_sample(sample).to_le_bytes()))
            .collect_vec();
        let number_of_channels = u16::try_from(channels.len()).ok().and_then(NonZeroU16::new).ok_or(FromSamplesError::InequalChannelLength)?;
        let bytes_per_sample = u16::try_from(size_of::<T>()).expect("size of sample type is too large");
        let mut data = Vec::new();
        loop {
            let samples = channels.iter_mut().map(Iterator::next).collect_vec();
            if samples.iter().all(Option::is_none) {
                break;
            }
            let Some(samples) = samples.into_iter().collect::<Option<Vec<_>>>() else {
                return Err(FromSamplesError::InequalChannelLength);
            };
            data.extend(samples.into_iter().flatten());
        }
        let data = data.into();

        Ok(Self {
            format,
            channels: number_of_channels,
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
        const RIFF_DATA_LEN_PCM: usize = 4 + 4 + 4 + 2 + 2 + 4 + 4 + 2 + 2 + 4 + 4;
        const RIFF_DATA_LEN_FLOAT: usize = RIFF_DATA_LEN_PCM + 2 + 4 + 4 + 4;
        writer.write_all(b"RIFF")?;
        writer.write_all(
            &u32::try_from(if self.format == Format::FloatingPoint { RIFF_DATA_LEN_FLOAT } else { RIFF_DATA_LEN_PCM } + self.data.len())
                .map_err(|_| WaveFileWriteError::DataTooLong)?
                .to_le_bytes(),
        )?;
        writer.write_all(b"WAVEfmt ")?;
        writer.write_all(&if self.format == Format::FloatingPoint { 18_u32 } else { 16_u32 }.to_le_bytes())?;
        writer.write_all(&(self.format as u16).to_le_bytes())?;
        writer.write_all(&self.channels.get().to_le_bytes())?;
        writer.write_all(&self.sample_rate.to_le_bytes())?;
        writer.write_all(&(self.sample_rate * u32::from(self.channels.get()) * u32::from(self.bytes_per_sample)).to_le_bytes())?;
        writer.write_all(&(self.bytes_per_sample).to_le_bytes())?;
        writer.write_all(&(self.bytes_per_sample * 8).to_le_bytes())?;
        if self.format == Format::FloatingPoint {
            writer.write_all(&0_u16.to_le_bytes())?;

            writer.write_all(b"fact")?;
            writer.write_all(&4_u32.to_le_bytes())?;
            writer.write_all(
                &u32::try_from(self.data.len() / self.bytes_per_sample as usize)
                    .map_err(|_| WaveFileWriteError::DataTooLong)?
                    .to_le_bytes(),
            )?;
        }
        writer.write_all(b"data")?;
        writer.write_all(&u32::try_from(self.data.len()).map_err(|_| WaveFileWriteError::DataTooLong)?.to_le_bytes())?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}
