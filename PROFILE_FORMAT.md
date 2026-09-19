# Trigram Profile Storage Format

This document describes how whatlang stores its language trigram profiles
(`src/trigrams/profiles.rs`), how they are loaded at runtime, and how the
file is regenerated.

## Goals of the layout

* Keep the release binary small: profiles are stored packed, not as
  `char` triples.
* Pay initialization cost only for languages that are actually examined:
  detecting with a whitelist (e.g. `Detector::with_allowlist`) never
  decodes profiles of other languages.
* Decoded profiles are reused safely across threads; the first concurrent
  access initializes a profile exactly once.
* No file I/O, no network access and no runtime (de)compression
  dependencies: all data is embedded in the binary as static arrays.

## Format version

The generated file defines:

```rust
pub const PROFILE_FORMAT_VERSION: u16 = 1;
```

Version 1 layout (all sizes in bytes refer to the compiled artifacts):

* For each multi-language script (`LATIN`, `CYRILLIC`, `ARABIC`,
  `DEVANAGARI`, `HEBREW`) there is one packed blob
  `static <SCRIPT>_DATA: &[u16]`. Every trigram is stored as 3 consecutive
  `u16` units; each unit is one Unicode BMP code point (`u16` little-endian
  for checksum purposes). The generator refuses characters above `U+FFFF`;
  supporting them would require a new format version.
* `static <SCRIPT>_INDEX: &[ProfileEntry]` with one entry per language, in
  the same order as the upstream `misc/data.json` (restricted to languages
  known to the `Lang` enum, minus `IGNORE_LANGS`):

  ```rust
  pub struct ProfileEntry {
      pub lang: Lang,
      pub offset: u32,   // start in <SCRIPT>_DATA, in u16 units
      pub len: u32,      // length in u16 units (3 per trigram)
      pub checksum: u32, // FNV-1a (32-bit) over the profile's u16 units
  }
  ```
* `static <SCRIPT>_CACHE: [OnceLock<LangProfile>; N]` — one lazily
  initialized slot per language.
* `pub static <SCRIPT>_LANGS: LangProfileList` ties blob, index and cache
  together.

The checksum is FNV-1a (32-bit): hash starts at `0x811c9dc5`; for every
`u16` unit its two little-endian bytes are folded in with
`hash = (hash ^ byte) * 0x01000193` (wrapping). The identical
implementation lives in `examples/generate_profiles.rs` (write side) and
in the generated `src/trigrams/profiles.rs` (verify side).

## Loading and concurrency

`LangProfileList::profile(index)` decodes the profile on first access:

1. `len` must be a multiple of 3.
2. The FNV-1a checksum of the `u16` slice must match `ProfileEntry.checksum`.
3. Each `u16` must be a valid `char`.

The decoded `&'static [Trigram]` is stored in the language's
`OnceLock` slot, so it is computed at most once per language per process,
even if several threads race on the first access, and is reused by all
later calls. Languages that are filtered out (whitelist/denylist) are
skipped before decoding, so their profiles are never touched.

## Corruption behavior

The blobs are compile-time constants, so corruption can only result from
a bad generator run or memory unsafety outside the library. The behavior
is fail-fast: any violation of the checks above **panics** with a message
naming the language, the expected and actual checksum and the format
version. The library never falls back to silently using partially decoded
or unverified data, because that could change detection results.
`trigrams::tests::test_corrupted_profile_panics` guards this behavior and
`trigrams::tests::test_profile_checksums_are_valid` verifies every shipped
profile against its checksum.

## Regenerating

From the repository root:

```text
cargo run --example generate_profiles
```

The generator reads `misc/data.json`, keeps the languages known to the
`Lang` enum (skipping `IGNORE_LANGS`, mirroring
`misc/update_support_languages.rb`), and rewrites
`src/trigrams/profiles.rs`. Output is deterministic: the same
`misc/data.json` produces a byte-identical file on repeated runs (guarded
by running the generator twice and comparing). The generated file is
rustfmt-clean; do not edit it by hand.

Detection results are guarded independently of the storage format: the
golden-master test `golden::tests::verify_golden` compares language,
script, confidence (exact `f64` bits) and the full trigram/combined
candidate rankings for the corpus (`tests/examples.json`) and the boundary
samples (`tests/boundary_samples.json`) against
`tests/golden_detect.json`. Regenerate the golden file only when a
behavior change is intended:

```text
cargo test generate_golden -- --ignored
```

## no_std notes

The blob format and the decoder use only `core` and `alloc`; the lazy
cache uses `std::sync::OnceLock`. The crate as a whole currently requires
`std` (as before this change); a `no_std` + `alloc` port only needs to
swap the once-initialization primitive (e.g. a spin-based `Once`). No
file-reading, networking or runtime-compression dependency was added.

## Size and performance

See BENCHMARKS.md for the measured profile-section size reduction,
steady-state throughput, first-call latency and peak memory, together
with the exact reproduction commands and raw tool output.
