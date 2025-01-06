use blerp::processing::{sawtooth_wave, triangle_wave};
use cpal::Sample;
use std::any::type_name_of_val;
use std::fs::{create_dir, remove_dir_all, File};

use blerp::{
    processing::{sine_wave, square_wave},
    wavefile::WaveFile,
};

#[test]
fn main() {
    const MIDDLE_C: f64 = 261.63;
    const SAMPLE_RATE: u32 = 44100;
    remove_dir_all(env!("CARGO_TARGET_TMPDIR")).unwrap();
    create_dir(env!("CARGO_TARGET_TMPDIR")).unwrap();
    macro_rules! test {
        ($fn:ident $($ty:ty)+) => {
            $(
                WaveFile::from_samples((0..44100).map(|sample| $fn::<$ty, 1>(MIDDLE_C, <$ty>::from_sample(1.))(f64::from(sample) / f64::from(SAMPLE_RATE))), SAMPLE_RATE)
                    .unwrap()
                    .write(&mut File::create(format!("{}{}{}{}", env!("CARGO_TARGET_TMPDIR"), "/", type_name_of_val(&$fn::<$ty, 1>), ".wav")).unwrap())
                    .unwrap();
            )+
        };
    }
    test!(sine_wave f32 f64 i8 i16 i32 i64 u8 u16 u32 u64);
    test!(square_wave f32 f64 i8 i16 i32 i64 u8 u16 u32 u64);
    test!(triangle_wave f32 f64 i8 i16 i32 i64 u8 u16 u32 u64);
    test!(sawtooth_wave f32 f64 i8 i16 i32 i64 u8 u16 u32 u64);
}
