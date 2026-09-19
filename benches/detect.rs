#[macro_use]
extern crate bencher;

use bencher::{Bencher, black_box};
use std::collections::BTreeMap;
use whatlang::{Detector, Lang, detect, detect_script};

fn example_texts() -> Vec<String> {
    let examples: BTreeMap<String, String> =
        serde_json::from_str(include_str!("../tests/examples.json")).unwrap();
    examples.into_values().collect()
}

// Detection over the full corpus with all languages enabled.
fn bench_detect_all_languages(bench: &mut Bencher) {
    let texts = example_texts();
    bench.iter(|| {
        for text in &texts {
            black_box(detect(black_box(text)));
        }
    })
}

// Detection restricted to a whitelist of three languages.
fn bench_detect_whitelist_3(bench: &mut Bencher) {
    let texts = [
        "There is no reason not to learn Esperanto.",
        "Mit dem Wissen wächst der Zweifel",
        "Además de todo lo anteriormente dicho, también encontramos...",
    ];
    let detector = Detector::with_allowlist(vec![Lang::Eng, Lang::Deu, Lang::Spa]);
    bench.iter(|| {
        for text in &texts {
            black_box(detector.detect(black_box(text)));
        }
    })
}

// Script detection fast path on short texts.
fn bench_detect_script_short_text(bench: &mut Bencher) {
    let texts = [
        "a",
        "ok",
        "the",
        "水",
        "Hello 世界",
        "😀🎉🚀",
        "नमस्ते दुनिया",
        "مرحبا بالعالم",
        "Mi ne scias!",
    ];
    bench.iter(|| {
        for text in &texts {
            black_box(detect_script(black_box(text)));
        }
    })
}

benchmark_group!(
    benches,
    bench_detect_all_languages,
    bench_detect_whitelist_3,
    bench_detect_script_short_text
);
benchmark_main!(benches);
