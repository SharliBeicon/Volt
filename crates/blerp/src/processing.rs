use std::ops::{Mul, Neg};

pub mod export;
pub mod generation;
pub mod live;

/// Return the `sample` clamped to between `threshold` and `-threshold` (inclusive).
///
/// # Panics
///
/// Panics if `threshold` is less than zero.
#[must_use]
pub fn clip<T: cpal::Sample + Ord + Neg<Output = T>>(sample: T, threshold: T) -> T {
    sample.clamp(-threshold, threshold)
}

/// Return the `sample` multiplied by `multiplier`.
#[must_use]
pub fn scale<T: cpal::Sample + Mul<Output = T>>(sample: T, multiplier: T) -> T {
    sample * multiplier
}
