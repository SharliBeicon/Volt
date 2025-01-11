use std::{
    borrow::Cow,
    fmt::Debug,
    io::{self, Write},
    iter::Iterator,
    num::NonZeroU16,
    option::Option,
};

use itertools::Itertools;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct WaveFile<'a> {
    pub channels: NonZeroU16,
    pub sample_rate: u32,
    pub data: Cow<'a, [u8]>,
}

#[derive(Error, Debug)]
pub enum WriteError {
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

impl WaveFile<'_> {
    /// Create a new [`WaveFile`] from an iterable of channels (where each channel is an iterable of samples) and a sample rate.
    /// # Errors
    /// If the number of channels does not fit in a [`NonZeroU16`], because it is zero, or more than [`u16::MAX`], return [`FromSamplesError::TooManyChannels`].
    /// If the channels are not all the same length, return [`FromSamplesError::InequalChannelLength`].
    pub fn from_samples<I: IntoIterator<Item = C>, C: IntoIterator<Item = f64>>(channels: I, sample_rate: u32) -> Result<Self, FromSamplesError> {
        let mut channels = channels
            .into_iter()
            .map(|channel| {
                channel.into_iter().map(|sample: f64| {
                    #[allow(clippy::cast_possible_truncation, reason = "truncation is expected, these are audio samples")]
                    (sample as f32).to_le_bytes()
                })
            })
            .collect_vec();
        let number_of_channels = u16::try_from(channels.len()).ok().and_then(NonZeroU16::new).ok_or(FromSamplesError::InequalChannelLength)?;
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
            channels: number_of_channels,
            sample_rate,
            data,
        })
    }

    /// Write the [`WaveFile`] to a writer.
    /// # Errors
    /// Returns an [`WaveFileWriteError::Io`] if writing to the writer fails (from calls to [`Write::write_all`]), or [`WaveFileWriteError::DataTooLong`] if the data was longer than [`u32::MAX`]
    /// bytes.
    pub fn write(&self, writer: &mut impl Write) -> Result<(), WriteError> {
        const RIFF_DATA_LEN_FLOAT: usize = 4 + 4 + 4 + 2 + 2 + 4 + 4 + 2 + 2 + 4 + 4 + 2 + 4 + 4 + 4;
        writer.write_all(b"RIFF")?;
        writer.write_all(&u32::try_from(RIFF_DATA_LEN_FLOAT + self.data.len()).map_err(|_| WriteError::DataTooLong)?.to_le_bytes())?;
        writer.write_all(b"WAVEfmt ")?;
        writer.write_all(&18_u32.to_le_bytes())?;
        writer.write_all(&3_u16.to_le_bytes())?;
        writer.write_all(&self.channels.get().to_le_bytes())?;
        writer.write_all(&self.sample_rate.to_le_bytes())?;
        writer.write_all(&(self.sample_rate * u32::from(self.channels.get()) * 8).to_le_bytes())?;
        writer.write_all(&4_u16.to_le_bytes())?;
        writer.write_all(&32_u16.to_le_bytes())?;
        writer.write_all(&0_u16.to_le_bytes())?;
        writer.write_all(b"fact")?;
        writer.write_all(&4_u32.to_le_bytes())?;
        writer.write_all(&u32::try_from(self.data.len() / 8).map_err(|_| WriteError::DataTooLong)?.to_le_bytes())?;
        writer.write_all(b"data")?;
        writer.write_all(&u32::try_from(self.data.len()).map_err(|_| WriteError::DataTooLong)?.to_le_bytes())?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}
