use cpal::Sample;
use std::any::type_name_of_val;
use std::fs::{create_dir, remove_dir_all, File};

use blerp::{
    processing::generation::{harmonics, sawtooth_wave, sine_wave, square_wave, triangle_wave, Harmonic},
    wavefile::WaveFile,
};

/// FIXME: this test produces some files which either
/// - cannot be played (some of the `i64`/`u64` files (does wave support 64-bit samples?))
/// - play the wrong sound (some of the `i8`/`u16`/`u32` files)
/// - completely kill macOS's audio system (coreaudiod) (some of the square wave files)
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

    // TODO test harmonic generation
    WaveFile::from_samples(
        (0..44100).map(|sample| harmonics::<f64, 1>(MIDDLE_C, &[Harmonic::new(1., 0), Harmonic::new(1., 1)])(f64::from(sample) / f64::from(SAMPLE_RATE))),
        SAMPLE_RATE,
    )
    .unwrap()
    .write(&mut File::create(format!("{}/harmonic_sin(x)+sin(2x)/2.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
    .unwrap();

    // panic to make the test fail (because it does) - see FIXME above
    panic!()
}
