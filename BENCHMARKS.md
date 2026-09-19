# Benchmarks: packed trigram profiles

Comparison of the profile storage refactor (packed `u16` blobs + lazy
per-language decoding) against the previous layout (`&'static [Trigram]`
tables, 12 bytes per trigram).

**Environment** (machine-dependent results): Apple M5 Max (arm64),
macOS 26.5.2, rustc 1.96.0, default features, `--release`.
Absolute numbers are only meaningful on this machine; rerun the commands
below to reproduce.

## Profile section size

Reproduce:

```text
cargo build --release --example cli
size -m target/release/examples/cli        # macOS; on Linux: size -A ...
```

Profile statics (exact accounting, platform independent):

| layout | trigram data | index table | total |
|---|---|---|---|
| before: 51 × 300 × `Trigram`(12 B) + `(Lang, &[Trigram])`(24 B) entries | 183,600 B | 1,224 B | 184,824 B |
| after: 51 × 300 × 3 × `u16`(6 B) + `ProfileEntry`(16 B) entries | 91,800 B | 816 B | 92,616 B |

→ **−49.9%** profile data (requirement: ≥ −20%).

Measured `__TEXT,__const` section of the release `cli` example
(contains the profile blobs plus unrelated constants):

| | `__const` |
|---|---|
| before | 204,700 B |
| after | 114,140 B |

→ **−44.2%** for the whole section. Raw summaries:

```text
# before (git HEAD)
Segment __TEXT: 557056
	Section __text: 286712
	Section __const: 204700
# after
Segment __TEXT: 475136
	Section __text: 288572
	Section __const: 114140
```

## Steady-state throughput

Reproduce: `cargo bench --bench detect` (baseline measured on git HEAD in
a worktree with the identical bench file). ns/iter, lower is better:

| bench | baseline (runs) | new (runs) | median Δ |
|---|---|---|---|
| `bench_detect_all_languages` | 3,231,670 / 2,524,569 / 2,511,770 | 2,567,162 / 2,612,542 / 2,564,840 / 2,486,599 | +1.6% |
| `bench_detect_whitelist_3` | 20,863 / 17,236 / 16,711 / 16,807 | 16,989 / 18,854 / 17,455 / 16,933 | −0.4% |
| `bench_detect_script_short_text` | 586 / 594 / 476 / 474 | 542 / 592 / 473 / 484 | −3.4% |

All deltas are within ±8% (requirement: no regression > 8%); run-to-run
noise on this machine is larger than the median deltas. The script fast
path does not touch profiles at all and is listed as a control.

Raw summary (one representative run per side):

```text
# baseline
test bench_detect_all_languages     ... bench:   2,511,770 ns/iter (+/- 318,399)
test bench_detect_script_short_text ... bench:         474 ns/iter (+/- 13)
test bench_detect_whitelist_3       ... bench:      16,807 ns/iter (+/- 351)
# new
test bench_detect_all_languages     ... bench:   2,486,599 ns/iter (+/- 57,083)
test bench_detect_script_short_text ... bench:         484 ns/iter (+/- 38)
test bench_detect_whitelist_3       ... bench:      16,933 ns/iter (+/- 431)
```

## First call and peak memory

Reproduce: `cargo run --release --example profile_metrics`
(counting global allocator; peak = high-water mark of live heap bytes).

| metric | baseline | new |
|---|---|---|
| first call, 3-language whitelist | 192 µs / 500 µs | 333 µs / 598 µs |
| first pass over full corpus (all languages) | 2.71 ms / 3.12 ms | 2.91 ms / 3.00 ms |
| peak heap after whitelist-3 call | 59.1 KiB | 62.0 KiB |
| peak heap after full corpus | 130.1 KiB | 309.4 KiB |

Notes:

* The first-call overhead is the one-time lazy decode: ~3 profiles
  (whitelist) or all 51 profiles (full corpus). It is paid once per
  process and only for languages actually examined.
* Peak heap after decoding all languages grows by ≈179 KiB — exactly the
  decoded profiles (51 × 300 × 12 B), which are kept for reuse. With the
  old layout the same bytes were instead resident as read-only data in
  the binary image; total footprint is comparable, but with a whitelist
  only the needed profiles are decoded (peak heap +2.9 KiB instead of
  the full 184 KiB profile table being mapped).

Raw summary (new layout):

```text
=== whatlang profile metrics ===
first_call_whitelist3:     598.25µs
first_call_all_corpus:    2.996458ms
peak_heap_after_whitelist3_kib:       62.0
peak_heap_after_all_kib:             309.4
steady_state_iters_per_sec:
  all_languages:        383.8
  whitelist_3:        56311.5
  script_short:     1935359.0
```

## Correctness guard

`cargo test --quiet` runs `golden::tests::verify_golden`: language,
script, confidence (exact `f64` bits — no floating-point representation
was changed) and the full trigram + combined candidate rankings are
identical to the pre-refactor baseline for every sample in
`tests/examples.json` and `tests/boundary_samples.json`.
