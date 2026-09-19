//! Benchmarks for the public detection API.
//!
//! Run from the repository root:
//!
//! ```sh
//! cargo bench --bench detect
//! ```
//!
//! Covers three scenarios:
//! * `bench_detect_all_languages` - full corpus, all languages enabled.
//! * `bench_detect_allowlist_3` - full corpus, only 3 languages allowed.
//! * `bench_detect_script_short_text` - short texts, script fast path.
//!
//! First-call latency and peak memory are measured separately, see
//! `examples/measure_startup.rs` and BENCHMARKS.md.

#[macro_use]
extern crate bencher;

use bencher::Bencher;
use std::collections::BTreeMap;
use whatlang::{Detector, Lang, detect, detect_script};

fn load_examples() -> BTreeMap<String, String> {
    let example_data = include_str!("../tests/examples.json");
    serde_json::from_str(example_data).unwrap()
}

// Detection over the whole example corpus with all languages enabled.
fn bench_detect_all_languages(bench: &mut Bencher) {
    let examples = load_examples();
    bench.iter(|| {
        for text in examples.values() {
            detect(text);
        }
    })
}

// Detection over the whole example corpus with an allowlist of 3 languages.
fn bench_detect_allowlist_3(bench: &mut Bencher) {
    let examples = load_examples();
    let detector = Detector::with_allowlist(vec![Lang::Eng, Lang::Spa, Lang::Deu]);
    bench.iter(|| {
        for text in examples.values() {
            detector.detect(text);
        }
    })
}

// Short texts, exercising the script detection fast path.
fn bench_detect_script_short_text(bench: &mut Bencher) {
    let texts = [
        "a",
        "ok",
        "oui",
        "no",
        "水",
        "こんにちは",
        "안녕하세요",
        "สวัสดี",
        "γειά",
        "مرحبا",
        "नमस्ते",
        "Привет",
        "Hello",
        "Mi ne scias!",
    ];
    bench.iter(|| {
        for text in texts {
            detect_script(text);
            detect(text);
        }
    })
}

benchmark_group!(
    benches,
    bench_detect_all_languages,
    bench_detect_allowlist_3,
    bench_detect_script_short_text
);
benchmark_main!(benches);
