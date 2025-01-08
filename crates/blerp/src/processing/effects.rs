use std::borrow::Cow;

use cpal::Sample;

use crate::Block;

/// An effect that can be applied to a sequence of blocks.
pub trait Effect<T: Sample, const N: usize> {
    type Error;
    /// Apply the effect to a sequence of blocks.
    /// # Errors
    /// If the effect fails to apply, return an error.
    fn apply<'a>(&self, input: Stuff<'a, T, N>) -> Result<Stuff<'a, T, N>, Self::Error>;
}

/// A structure that holds the time, sample rate, and blocks to be processed.
pub struct Stuff<'a, T: Sample, const N: usize> {
    /// Time used to find a block
    pub time: f64,
    /// Sample rate used in conjunction with [`Self::time`] to find a block
    pub sample_rate: f64,
    /// Sequence of blocks
    pub blocks: Cow<'a, [Block<T, N>]>,
}

pub mod clip {
    use std::convert::Infallible;

    use super::{Effect, Stuff};
    use crate::Block;
    use cpal::{FromSample, Sample};
    use itertools::Itertools;

    /// An effect that clips a sample to between `lower` and `upper`.
    pub struct Clip<T: Sample> {
        lower: T,
        upper: T,
    }

    impl<T: Sample + Ord, const N: usize> Effect<T, N> for Clip<T> {
        type Error = Infallible;

        fn apply<'a>(&self, mut stuff: Stuff<'a, T, N>) -> Result<Stuff<'a, T, N>, Self::Error> {
            stuff.blocks = stuff
                .blocks
                .iter()
                .map(|Block(block)| Block(block.map(|sample| sample.clamp(self.lower, self.upper))))
                .collect_vec()
                .into();
            Ok(stuff)
        }
    }

    impl<T: Sample> Clip<T> {
        /// Return a new [`Clip`] which clips samples to between `lower` and `upper`.
        ///
        /// If `lower` is greater than `upper`, swap them so that [`Ord::clamp`] will not panic.
        pub fn new(lower: T, upper: T) -> Self {
            let (lower, upper) = if lower > upper { (upper, lower) } else { (lower, upper) };
            Self { lower, upper }
        }

        /// Return a new [`Clip`] which clips samples to between `-absolute` and `absolute`.
        /// If the absolute is negative, it will be made positive so that [`Ord::clamp`] will not panic.
        pub fn new_symmetrical(mut absolute: T) -> Self
        where
            T: FromSample<f64>,
            f64: FromSample<T>,
        {
            if absolute < T::from_sample(0.0) {
                absolute = T::from_sample(-f64::from_sample(absolute));
            }
            Self {
                lower: T::from_sample(-f64::from_sample(absolute)),
                upper: absolute,
            }
        }
    }
}

pub mod scale {
    use std::convert::Infallible;

    use super::{Effect, Stuff};
    use crate::Block;
    use cpal::{FromSample, Sample};
    use itertools::Itertools;

    /// An effect that scales a sample by a factor.
    pub struct Scale {
        factor: f64,
    }

    impl<T: Sample + FromSample<f64>, const N: usize> Effect<T, N> for Scale
    where
        f64: FromSample<T>,
    {
        type Error = Infallible;

        fn apply<'a>(&self, mut stuff: Stuff<'a, T, N>) -> Result<Stuff<'a, T, N>, Self::Error> {
            stuff.blocks = stuff
                .blocks
                .iter()
                .map(|Block(block)| Block(block.map(|sample| T::from_sample(f64::from_sample(sample) * self.factor))))
                .collect_vec()
                .into();
            Ok(stuff)
        }
    }

    impl Scale {
        /// Return a new [`Scale`] which scales samples by `factor`.
        #[must_use]
        pub const fn new(factor: f64) -> Self {
            Self { factor }
        }
    }
}
