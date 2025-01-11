use std::fs::{create_dir, remove_dir_all, File};

use blerp::{
    processing::generation::{harmonics, sawtooth_wave, sine_wave, square_wave, triangle_wave, Harmonic},
    wavefile::WaveFile,
};
use itertools::Itertools;

#[test]
fn main() {
    const MIDDLE_C: f64 = 261.63;
    const SAMPLE_RATE: u32 = 44100;
    remove_dir_all(env!("CARGO_TARGET_TMPDIR")).unwrap();
    create_dir(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let types = ["u8", "i16", "i32", "i64", "f32", "f64"];
    for (index, from_samples) in [
        WaveFile::from_samples::<u8, _>,
        WaveFile::from_samples::<i16, _>,
        WaveFile::from_samples::<i32, _>,
        WaveFile::from_samples::<i64, _>,
        WaveFile::from_samples::<f32, _>,
        WaveFile::from_samples::<f64, _>,
    ]
    .into_iter()
    .enumerate()
    {
        let r#type = types[index];
        from_samples(
            [(0..44100).map(|sample| sine_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE))).collect_vec()],
            SAMPLE_RATE,
        )
        .unwrap()
        .write(&mut File::create(format!("{}/sine_wave{type}.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
        from_samples(
            [(0..44100).map(|sample| sawtooth_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE))).collect_vec()],
            SAMPLE_RATE,
        )
        .unwrap()
        .write(&mut File::create(format!("{}/sawtooth_wave{type}.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
        from_samples(
            [(0..44100).map(|sample| triangle_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE))).collect_vec()],
            SAMPLE_RATE,
        )
        .unwrap()
        .write(&mut File::create(format!("{}/triangle_wave{type}.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
        from_samples(
            [(0..44100).map(|sample| square_wave(MIDDLE_C, 1.)(f64::from(sample) / f64::from(SAMPLE_RATE))).collect_vec()],
            SAMPLE_RATE,
        )
        .unwrap()
        .write(&mut File::create(format!("{}/square_wave{type}.wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
        .unwrap();
    }

    WaveFile::from_samples::<f32, _>(
        [(0..44100).map(|sample| harmonics(MIDDLE_C, &[Harmonic::new(1., 0), Harmonic::new(1., 1)])(f64::from(sample) / f64::from(SAMPLE_RATE)))],
        SAMPLE_RATE,
    )
    .unwrap()
    .write(&mut File::create(format!("{}/harmonic_sin(x)+0.5sin(2x).wav", env!("CARGO_TARGET_TMPDIR"))).unwrap())
    .unwrap();
}
