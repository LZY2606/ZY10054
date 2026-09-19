//! Regenerates `src/trigrams/profiles.rs` from `misc/data.json`.
//!
//! Run from the repository root:
//!
//! ```text
//! cargo run --example generate_profiles
//! ```
//!
//! The output is deterministic: the same `misc/data.json` always produces
//! a byte-identical `src/trigrams/profiles.rs`. See PROFILE_FORMAT.md.

use serde_json::Value;
use whatlang::Lang;

// (key in misc/data.json, name of the generated constants)
const GROUPS: [(&str, &str); 5] = [
    ("Latin", "LATIN"),
    ("Cyrillic", "CYRILLIC"),
    ("Arabic", "ARABIC"),
    ("Devanagari", "DEVANAGARI"),
    ("Hebrew", "HEBREW"),
];

const UNITS_PER_LINE: usize = 16;

// Languages excluded from profile generation, matching
// IGNORE_LANGS in misc/update_support_languages.rb.
const IGNORE_LANGS: &[(&str, &[&str])] = &[
    // Do not generate cyrillic trigrams for Turkmen and Azerbaijani
    ("Cyrillic", &["tuk", "aze"]),
];

// FNV-1a (32-bit) over u16 units consumed as little-endian byte pairs.
// Must stay in sync with `fnv1a_32` in the generated src/trigrams/profiles.rs.
fn fnv1a_32(data: &[u16]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &unit in data {
        for byte in unit.to_le_bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    hash
}

fn main() {
    let raw = include_str!("../misc/data.json");
    let data: Value = serde_json::from_str(raw).expect("misc/data.json must be valid JSON");

    let mut out = String::new();
    out.push_str(HEADER);

    for (json_key, const_name) in GROUPS {
        let langs = data
            .get(json_key)
            .unwrap_or_else(|| panic!("misc/data.json misses script {json_key:?}"))
            .as_object()
            .expect("script entry must be an object");

        let mut units: Vec<u16> = Vec::new();
        let mut entries: Vec<(Lang, u32, u32, u32)> = Vec::new();

        let ignore_langs = IGNORE_LANGS
            .iter()
            .find(|(script, _)| *script == json_key)
            .map(|(_, codes)| *codes)
            .unwrap_or(&[]);
        for (code, trigrams) in langs {
            if ignore_langs.contains(&code.as_str()) {
                continue;
            }
            // Only languages known to the `Lang` enum are embedded.
            let Some(lang) = Lang::from_code(code) else {
                continue;
            };
            let trigrams = trigrams.as_str().expect("trigrams must be a string");
            let offset = units.len() as u32;
            for trigram in trigrams.split('|').filter(|t| !t.is_empty()) {
                let chars: Vec<char> = trigram.chars().collect();
                assert_eq!(
                    chars.len(),
                    3,
                    "trigram {trigram:?} of language {code} must have exactly 3 chars"
                );
                for ch in chars {
                    let value = ch as u32;
                    assert!(
                        value <= 0xFFFF,
                        "char {ch:?} of language {code} does not fit into u16; \
                         the profile format must be extended (bump PROFILE_FORMAT_VERSION)"
                    );
                    units.push(value as u16);
                }
            }
            let len = units.len() as u32 - offset;
            let checksum = fnv1a_32(&units[offset as usize..]);
            entries.push((lang, offset, len, checksum));
        }

        emit_group(&mut out, json_key, const_name, &units, &entries);
    }

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/trigrams/profiles.rs");
    std::fs::write(path, format!("{}\n", out.trim_end())).unwrap();
    println!("wrote {path}");
}

fn emit_group(
    out: &mut String,
    display_name: &str,
    name: &str,
    units: &[u16],
    entries: &[(Lang, u32, u32, u32)],
) {
    out.push_str(&format!(
        "#[rustfmt::skip]\nstatic {name}_DATA: &[u16] = &[\n"
    ));
    for chunk in units.chunks(UNITS_PER_LINE) {
        out.push_str("    ");
        for unit in chunk {
            out.push_str(&format!("{unit}, "));
        }
        out.push('\n');
    }
    out.push_str("];\n\n");

    out.push_str(&format!(
        "#[rustfmt::skip]\nstatic {name}_INDEX: &[ProfileEntry] = &[\n"
    ));
    for (lang, offset, len, checksum) in entries {
        out.push_str(&format!(
            "    ProfileEntry {{ lang: Lang::{lang:?}, offset: {offset}, len: {len}, checksum: {checksum:#010x} }},\n"
        ));
    }
    out.push_str("];\n\n");

    let count = entries.len();
    out.push_str(&format!("#[rustfmt::skip]\nstatic {name}_CACHE: [OnceLock<LangProfile>; {count}] = [const {{ OnceLock::new() }}; {count}];\n\n"));
    out.push_str(&format!("/// Languages for script {display_name}\n"));
    out.push_str(&format!("#[rustfmt::skip]\npub static {name}_LANGS: LangProfileList = LangProfileList {{\n    data: {name}_DATA,\n    index: {name}_INDEX,\n    cache: &{name}_CACHE,\n}};\n\n"));
}

const HEADER: &str = r#"// NOTE:
//    This file is generated automatically.
//    Run `cargo run --example generate_profiles` to regenerate it from misc/data.json.
//    See PROFILE_FORMAT.md for the storage format, checksums and corruption behavior.

use crate::Lang;
use crate::trigrams::Trigram;
use std::sync::OnceLock;

/// Version of the profile storage format emitted by the generator.
pub const PROFILE_FORMAT_VERSION: u16 = 1;

/// A decoded language profile: trigrams ordered by decreasing frequency.
pub type LangProfile = &'static [Trigram];

/// Index entry locating one language profile inside a packed data blob.
#[derive(Debug, Clone, Copy)]
pub struct ProfileEntry {
    pub lang: Lang,
    /// Offset in u16 units inside the blob.
    pub offset: u32,
    /// Length in u16 units (3 units per trigram).
    pub len: u32,
    /// FNV-1a (32-bit) checksum over the profile's u16 units.
    pub checksum: u32,
}

/// FNV-1a (32-bit) over u16 units consumed as little-endian byte pairs.
/// Must stay in sync with `fnv1a_32` in examples/generate_profiles.rs.
pub(crate) fn fnv1a_32(data: &[u16]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &unit in data {
        for byte in unit.to_le_bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    hash
}

/// Packed, lazily decoded list of language profiles for one script.
///
/// Trigrams are stored as u16 triples (one Unicode BMP code point per unit)
/// and decoded into `LangProfile` on first access, once per language.
pub struct LangProfileList {
    data: &'static [u16],
    index: &'static [ProfileEntry],
    cache: &'static [OnceLock<LangProfile>],
}

impl LangProfileList {
    #[inline]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Language of the profile at `index`. Does not trigger decoding.
    #[inline]
    pub fn lang(&self, index: usize) -> Lang {
        self.index[index].lang
    }

    /// Decoded profile at `index`.
    ///
    /// The profile is decoded at most once per language, even when several
    /// threads race on the first access; the result is reused afterwards.
    #[inline]
    pub fn profile(&self, index: usize) -> LangProfile {
        self.cache[index].get_or_init(|| self.decode(index))
    }

    fn decode(&self, index: usize) -> LangProfile {
        let entry = &self.index[index];
        assert!(
            entry.len.is_multiple_of(3),
            "whatlang: corrupted trigram profile for {:?}: length {} is not a multiple of 3",
            entry.lang,
            entry.len
        );
        let start = entry.offset as usize;
        let end = start + entry.len as usize;
        let raw = &self.data[start..end];
        let checksum = fnv1a_32(raw);
        assert!(
            checksum == entry.checksum,
            "whatlang: corrupted trigram profile for {:?}: checksum mismatch \
             (expected {:#010x}, got {:#010x}, format version {})",
            entry.lang,
            entry.checksum,
            checksum,
            PROFILE_FORMAT_VERSION
        );
        let trigrams: Vec<Trigram> = raw
            .chunks_exact(3)
            .map(|unit| {
                let to_char = |v: u16| {
                    char::from_u32(u32::from(v)).expect(
                        "whatlang: corrupted trigram profile: invalid char (checksum passed)",
                    )
                };
                Trigram(to_char(unit[0]), to_char(unit[1]), to_char(unit[2]))
            })
            .collect();
        Box::leak(trigrams.into_boxed_slice())
    }

    #[cfg(test)]
    pub(crate) fn is_initialized(&self, index: usize) -> bool {
        self.cache[index].get().is_some()
    }

    #[cfg(test)]
    pub(crate) fn parts(&self) -> (&'static [u16], &'static [ProfileEntry]) {
        (self.data, self.index)
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        data: &'static [u16],
        index: &'static [ProfileEntry],
        cache: &'static [OnceLock<LangProfile>],
    ) -> Self {
        Self { data, index, cache }
    }
}

"#;
