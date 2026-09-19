//! Measures first-call latency and steady-state throughput of detection.
//!
//! Run from the repository root (release mode matters):
//!
//! ```sh
//! cargo run --release --example measure_startup --features dev -- all
//! cargo run --release --example measure_startup --features dev -- allow3
//! ```
//!
//! To measure peak memory of the same scenarios on macOS:
//!
//! ```sh
//! /usr/bin/time -l ./target/release/examples/measure_startup all
//! /usr/bin/time -l ./target/release/examples/measure_startup allow3
//! ```
//!
//! Modes:
//! * `all`    - detection with all languages enabled (default).
//! * `allow3` - detection with an allowlist of 3 languages.
//!
use std::hint::black_box;
use std::time::Instant;
use whatlang::dev;
use whatlang::{Detector, Lang, detect};

const TEXT: &str = "Esperanto is a constructed international auxiliary language. \
    It was created in the late nineteenth century and is now spoken by people \
    all over the world. This sentence is long enough to give the detector \
    a realistic amount of trigram data to work with.";

const ITERATIONS: u32 = 10_000;

fn report(mode: &str, first_call: std::time::Duration, steady: std::time::Duration) {
    println!("mode={mode}");
    println!("first_call_ns={}", first_call.as_nanos());
    println!("steady_state_ns_per_call={}", steady.as_nanos());
    println!("embedded_profile_bytes={}", dev::profile_storage_bytes());
    println!("decoded_profile_bytes={}", dev::decoded_profile_bytes());
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    match mode.as_str() {
        "all" => {
            let start = Instant::now();
            let info = detect(TEXT).expect("detection failed");
            let first_call = start.elapsed();

            let start = Instant::now();
            for _ in 0..ITERATIONS {
                black_box(detect(black_box(TEXT)));
            }
            let steady = start.elapsed() / ITERATIONS;

            assert_eq!(info.lang(), Lang::Eng);
            report("all", first_call, steady);
        }
        "allow3" => {
            let detector = Detector::with_allowlist(vec![Lang::Eng, Lang::Spa, Lang::Deu]);

            let start = Instant::now();
            let info = detector.detect(TEXT).expect("detection failed");
            let first_call = start.elapsed();

            let start = Instant::now();
            for _ in 0..ITERATIONS {
                black_box(detector.detect(black_box(TEXT)));
            }
            let steady = start.elapsed() / ITERATIONS;

            assert_eq!(info.lang(), Lang::Eng);
            report("allow3", first_call, steady);
        }
        other => {
            eprintln!("unknown mode: {other} (expected 'all' or 'allow3')");
            std::process::exit(2);
        }
    }
}
