//! Reports first-call latency, steady-state throughput and peak heap usage
//! for the language detection pipeline.
//!
//! Run with:
//!
//! ```text
//! cargo run --release --example profile_metrics
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use whatlang::{Detector, Lang, detect, detect_script};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn peak_kib() -> f64 {
    PEAK_BYTES.load(Ordering::Relaxed) as f64 / 1024.0
}

fn example_texts() -> Vec<String> {
    let examples: BTreeMap<String, String> =
        serde_json::from_str(include_str!("../tests/examples.json")).unwrap();
    examples.into_values().collect()
}

fn throughput<F: FnMut()>(mut f: F) -> f64 {
    // Warm up, then measure.
    for _ in 0..3 {
        f();
    }
    let iterations = 20;
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    iterations as f64 / elapsed.as_secs_f64()
}

fn main() {
    let texts = example_texts();
    let whitelist_detector = Detector::with_allowlist(vec![Lang::Eng, Lang::Deu, Lang::Spa]);
    let whitelist_texts = [
        "There is no reason not to learn Esperanto.",
        "Mit dem Wissen wächst der Zweifel",
        "Además de todo lo anteriormente dicho, también encontramos...",
    ];
    let short_texts = [
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

    // Phase 1: cold first call with a 3-language whitelist.
    let start = Instant::now();
    let info = whitelist_detector.detect(whitelist_texts[0]);
    let first_call_whitelist3 = start.elapsed();
    assert_eq!(info.unwrap().lang(), Lang::Eng);
    let peak_after_whitelist3 = peak_kib();

    // Phase 2: first call over the whole corpus (all remaining languages).
    let start = Instant::now();
    for text in &texts {
        std::hint::black_box(detect(std::hint::black_box(text)));
    }
    let first_call_all = start.elapsed();
    let peak_after_all = peak_kib();

    // Phase 3: steady-state throughput.
    let all_per_sec = throughput(|| {
        for text in &texts {
            std::hint::black_box(detect(std::hint::black_box(text)));
        }
    });
    let whitelist_per_sec = throughput(|| {
        for text in &whitelist_texts {
            std::hint::black_box(whitelist_detector.detect(std::hint::black_box(text)));
        }
    });
    let script_per_sec = throughput(|| {
        for text in &short_texts {
            std::hint::black_box(detect_script(std::hint::black_box(text)));
        }
    });

    println!("=== whatlang profile metrics ===");
    println!("first_call_whitelist3:  {first_call_whitelist3:>12?}");
    println!("first_call_all_corpus:  {first_call_all:>12?}");
    println!("peak_heap_after_whitelist3_kib: {peak_after_whitelist3:>10.1}");
    println!("peak_heap_after_all_kib:        {peak_after_all:>10.1}");
    println!("steady_state_iters_per_sec:");
    println!("  all_languages:   {all_per_sec:>10.1}");
    println!("  whitelist_3:     {whitelist_per_sec:>10.1}");
    println!("  script_short:    {script_per_sec:>10.1}");
}
