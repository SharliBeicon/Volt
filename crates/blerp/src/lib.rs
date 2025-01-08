#![warn(clippy::nursery, clippy::pedantic, clippy::undocumented_unsafe_blocks)]
use std::{iter::Sum, ops::Div};

use cpal::{FromSample, Sample};
use itertools::Itertools;

pub mod device;
pub mod processing;
pub mod wavefile;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Block<T: Sample, const N: usize>([T; N]);

impl<T: Sample> From<T> for Block<T, 1> {
    fn from(value: T) -> Self {
        Self([value])
    }
}

impl<T: Sample, const N: usize> From<[T; N]> for Block<T, N> {
    fn from(value: [T; N]) -> Self {
        Self(value)
    }
}

impl<T: Sample + FromSample<f64>, const N: usize> Div<T> for Block<T, N>
where
    f64: FromSample<T>,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        Self(self.0.map(|sample| T::from_sample(f64::from_sample(sample) / f64::from_sample(rhs))))
    }
}

impl<T: Sample + FromSample<f64>, const N: usize> Sum for Block<T, N>
where
    f64: FromSample<T>,
{
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold([T::EQUILIBRIUM; N], |a, Self(b)| {
            // SAFETY: The iterator is produced from `a` and `b`, which are both arrays of size `N`, so the iterator has `N` elements.
            // Also, `zip_eq` does not panic because `a` and `b` are both arrays of size `N`.
            unsafe {
                a.into_iter()
                    .zip_eq(b)
                    .map(|(a, b)| T::from_sample(f64::from_sample(a) + f64::from_sample(b)))
                    .collect_array()
                    .unwrap_unchecked()
            }
        })
        .into()
    }
}
