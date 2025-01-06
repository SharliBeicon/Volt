pub mod device;
pub mod processing;
pub mod wavefile;

pub struct Sample<T: cpal::Sample, const N: usize>([T; N]);

impl<T: cpal::Sample> From<T> for Sample<T, 1> {
    fn from(value: T) -> Self {
        Self([value])
    }
}
