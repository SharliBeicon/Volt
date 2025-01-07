use std::f64::consts::TAU;

use cpal::{FromSample, Sample};

use crate::Block;

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a sine wave.
pub fn sine_wave<T: Sample + FromSample<f64>, const N: usize>(frequency: f64, amplitude: T) -> impl FnMut(f64) -> Block<T, N>
where
    f64: FromSample<T>,
{
    move |time| Block([T::from_sample(f64::from_sample(amplitude) * (TAU * frequency * time).sin()); N])
}

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a square wave.
pub fn square_wave<T: Sample + FromSample<f64>, const N: usize>(frequency: f64, amplitude: T) -> impl FnMut(f64) -> Block<T, N>
where
    f64: FromSample<T>,
{
    move |time| Block([T::from_sample((-1_f64).powf(2. * frequency * time) * f64::from_sample(amplitude)); N])
}

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a triangle wave.
pub fn triangle_wave<T: Sample + FromSample<f64>, const N: usize>(frequency: f64, amplitude: T) -> impl FnMut(f64) -> Block<T, N>
where
    f64: FromSample<T>,
{
    move |time| Block([T::from_sample((2. * f64::from_sample(amplitude)) * time.mul_add(frequency, -time.mul_add(frequency, 1. / 2.).floor()).abs()); N])
}

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a sawtooth wave.
pub fn sawtooth_wave<T: Sample + FromSample<f64>, const N: usize>(frequency: f64, amplitude: T) -> impl FnMut(f64) -> Block<T, N>
where
    f64: FromSample<T>,
{
    move |time| Block([T::from_sample((2. * f64::from_sample(amplitude)) * time.mul_add(frequency, -time.mul_add(frequency, 1. / 2.).floor())); N])
}

/// Return a function that generates silence.
pub fn silence<T: Sample, const N: usize>() -> impl FnMut(f64) -> Block<T, N> {
    move |_| Block([T::EQUILIBRIUM; N])
}

#[derive(Clone, Copy, Debug)]
/// A sine wave with a given `amplitude` and `index`.
///
/// The `index` is used to determine the harmonic frequency of the wave. See [`harmonics`] for more information.
pub struct Harmonic<T: Sample> {
    amplitude: T,
    index: usize,
}

impl<T: Sample> Harmonic<T> {
    /// Create a new harmonic with the given `amplitude` and `index`.
    pub const fn new(amplitude: T, index: usize) -> Self {
        Self { amplitude, index }
    }
}

/// Return a function over time (in seconds) that generates a wave resulting from the sum of the given harmonics.
///
/// Each harmonic is a sine wave with a given `amplitude` and `index`, with a frequency of `(index + 1) * fundamental_frequency`.
pub fn harmonics<T: Sample + FromSample<f64>, const N: usize>(fundamental_frequency: f64, harmonics: &[Harmonic<T>]) -> impl FnMut(f64) -> Block<T, N> + use<'_, T, N>
where
    f64: FromSample<T>,
{
    move |time| {
        #[allow(clippy::cast_precision_loss)]
        harmonics
            .iter()
            .map(|harmonic| sine_wave((harmonic.index as f64 + 1.) * fundamental_frequency, harmonic.amplitude)(time) / T::from_sample(harmonic.index as f64 + 1.))
            .sum()
    }
}
