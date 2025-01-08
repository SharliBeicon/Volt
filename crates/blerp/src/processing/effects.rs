use std::{borrow::Cow, fmt::Display};

/// An effect that can be applied to a sequence of blocks.
pub trait Effect: Display {
    /// Apply the effect to a sequence of blocks.
    /// # Errors
    /// If the effect fails to apply, return an error.
    fn apply<'a>(&self, input: Stuff<'a>) -> Result<Stuff<'a>, EffectError>;
}

pub enum EffectError {}

/// A structure that holds the time, sample rate, and samples to be processed.
pub struct Stuff<'a> {
    /// Time used to find a block.
    pub time: f64,
    /// Sample rate used in conjunction with [`Self::time`] to find a block.
    pub sample_rate: f64,
    /// Sequence of samples.
    pub samples: Cow<'a, [f64]>,
}

pub mod clip {
    use std::fmt::{self, Display, Formatter};

    use super::{Effect, EffectError, Stuff};
    use cpal::Sample;
    use itertools::Itertools;

    /// An effect that clips a sample to between `lower` and `upper`.
    pub struct Clip {
        lower: f64,
        upper: f64,
    }

    impl Effect for Clip {
        fn apply<'a>(&self, mut input: Stuff<'a>) -> Result<Stuff<'a>, EffectError> {
            input.samples = input.samples.iter().map(|sample| sample.clamp(self.lower, self.upper)).collect_vec().into();
            Ok(input)
        }
    }

    impl Display for Clip {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Clip")
        }
    }

    impl Clip {
        /// Return a new [`Clip`] which clips samples to between `lower` and `upper`.
        ///
        /// If `lower` is greater than `upper`, swap them so that [`Ord::clamp`] will not panic.
        #[must_use]
        pub fn new(lower: f64, upper: f64) -> Self {
            let (lower, upper) = if lower > upper { (upper, lower) } else { (lower, upper) };
            Self { lower, upper }
        }

        /// Return a new [`Clip`] which clips samples to between `-absolute` and `absolute`.
        /// If the absolute is negative, it will be made positive so that [`Ord::clamp`] will not panic.
        #[must_use]
        pub fn new_symmetrical(mut absolute: f64) -> Self {
            if absolute < (0.0) {
                absolute = -f64::from_sample(absolute);
            }
            Self {
                lower: (-f64::from_sample(absolute)),
                upper: absolute,
            }
        }
    }
}

pub mod scale {
    use std::fmt::{self, Display, Formatter};

    use super::{Effect, EffectError, Stuff};
    use itertools::Itertools;

    /// An effect that scales a sample by a factor.
    pub struct Scale {
        factor: f64,
    }

    impl Display for Scale {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Scale")
        }
    }

    impl Effect for Scale {
        fn apply<'a>(&self, mut input: Stuff<'a>) -> Result<Stuff<'a>, EffectError> {
            input.samples = input.samples.iter().map(|sample| sample * self.factor).collect_vec().into();
            Ok(input)
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
