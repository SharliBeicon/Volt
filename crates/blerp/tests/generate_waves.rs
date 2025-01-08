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
    WaveFile::from_samples([(0..44100).map(|sample| sine_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE)))], SAMPLE_RATE)
        .unwrap()
        .write(&mut File::create(format!("{}/sine_wave.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
    WaveFile::from_samples([(0..44100).map(|sample| sawtooth_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE)))], SAMPLE_RATE)
        .unwrap()
        .write(&mut File::create(format!("{}/sine_wave.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
    WaveFile::from_samples([(0..44100).map(|sample| triangle_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE)))], SAMPLE_RATE)
        .unwrap()
        .write(&mut File::create(format!("{}/sine_wave.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
    WaveFile::from_samples([(0..44100).map(|sample| square_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE)))], SAMPLE_RATE)
        .unwrap()
        .write(&mut File::create(format!("{}/sine_wave.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();

    // TODO test harmonic generation
    WaveFile::from_samples(
        [(0..44100).map(|sample| harmonics(MIDDLE_C, &[Harmonic::new(1., 0), Harmonic::new(1., 1)])(f64::from(sample) / f64::from(SAMPLE_RATE)))],
        SAMPLE_RATE,
    )
    .unwrap()
    .write(&mut File::create(format!("{}/harmonic_sin(x)+sin(2x)/2.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
    .unwrap();

    // panic to make the test fail (because it does) - see FIXME above
    panic!()
}
