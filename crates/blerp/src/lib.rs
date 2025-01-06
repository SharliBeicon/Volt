use cpal::Sample;

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
