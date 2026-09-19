pub mod detection;
mod profiles;
pub mod utils;

pub use profiles::*;

pub use detection::{RawOutcome, detect, raw_detect};

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Clone, Copy)]
pub struct Trigram(pub(crate) char, pub(crate) char, pub(crate) char);

// Maximum distance(difference) for a trigram in a language profile and text profile.
pub const MAX_TRIGRAM_DISTANCE: u32 = 300;

// 300 trigrams where each has MAX_TOTAL_DISTANCE=300, gives us 90_000.
pub const MAX_TOTAL_DISTANCE: u32 = MAX_TRIGRAM_DISTANCE * MAX_TRIGRAM_DISTANCE;

// Double MAX_TRIGRAM_DISTANCE
pub const TEXT_TRIGRAMS_SIZE: usize = 600;

#[cfg(test)]
mod tests {
    use super::detection::calculate_scores_in_profiles;
    use super::profiles::{
        ARABIC_LANGS, CYRILLIC_LANGS, DEVANAGARI_LANGS, HEBREW_LANGS, LATIN_LANGS, fnv1a_32,
    };
    use super::{LangProfile, LangProfileList, ProfileEntry};
    use crate::Lang;
    use crate::core::{FilterList, Text};
    use std::sync::OnceLock;

    // `cargo test --quiet` hides test names; announce validation stages
    // on stderr (which is not captured) so they stay visible.
    fn announce_stage(name: &str) {
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), "stage: trigrams::tests::{name}");
    }

    fn all_profile_lists() -> [&'static LangProfileList; 5] {
        [
            &LATIN_LANGS,
            &CYRILLIC_LANGS,
            &ARABIC_LANGS,
            &DEVANAGARI_LANGS,
            &HEBREW_LANGS,
        ]
    }

    // Decoding verifies the per-language checksum; this would panic on
    // any mismatch between the index and the packed data.
    #[test]
    fn test_profile_checksums_are_valid() {
        announce_stage("test_profile_checksums_are_valid");
        for list in all_profile_lists() {
            for index in 0..list.len() {
                assert!(!list.profile(index).is_empty());
            }
        }
    }

    #[test]
    #[should_panic(expected = "checksum mismatch")]
    fn test_corrupted_profile_panics() {
        announce_stage("test_corrupted_profile_panics");
        static DATA: &[u16] = &[0x0020, 0x0064, 0x0065]; // " de"
        let bad_checksum = fnv1a_32(DATA) ^ 1;
        let index: &'static [ProfileEntry] = Box::leak(
            vec![ProfileEntry {
                lang: Lang::Eng,
                offset: 0,
                len: 3,
                checksum: bad_checksum,
            }]
            .into_boxed_slice(),
        );
        let cache: &'static [OnceLock<LangProfile>] =
            Box::leak(vec![OnceLock::new()].into_boxed_slice());
        let list = LangProfileList::new_for_test(DATA, index, cache);
        let _ = list.profile(0);
    }

    // A fresh list over the real Latin data: only whitelisted languages
    // may be decoded.
    #[test]
    fn test_whitelist_decodes_only_allowed_languages() {
        announce_stage("test_whitelist_decodes_only_allowed_languages");
        let (data, index) = LATIN_LANGS.parts();
        let cache: &'static [OnceLock<LangProfile>] = Box::leak(
            (0..index.len())
                .map(|_| OnceLock::new())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let list = LangProfileList::new_for_test(data, index, cache);

        let text = Text::new("There is no reason not to learn Esperanto.");
        let filter_list = FilterList::allow(vec![Lang::Eng, Lang::Deu, Lang::Spa]);
        let outcome = calculate_scores_in_profiles(&text, &filter_list, &list);

        assert_eq!(outcome.scores.len(), 3);
        let decoded: Vec<Lang> = (0..list.len())
            .filter(|&index| list.is_initialized(index))
            .map(|index| list.lang(index))
            .collect();
        // Decoding happens in profile index order, not allowlist order.
        assert_eq!(decoded, vec![Lang::Spa, Lang::Eng, Lang::Deu]);
    }

    // Racing threads must observe a single decoded profile instance.
    #[test]
    fn test_concurrent_first_access_initializes_once() {
        announce_stage("test_concurrent_first_access_initializes_once");
        let (data, index) = LATIN_LANGS.parts();
        let cache: &'static [OnceLock<LangProfile>] = Box::leak(
            (0..index.len())
                .map(|_| OnceLock::new())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let list: &'static LangProfileList =
            Box::leak(Box::new(LangProfileList::new_for_test(data, index, cache)));

        let addresses: Vec<usize> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| list.profile(0).as_ptr() as usize))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(list.is_initialized(0));
        assert!(
            addresses.windows(2).all(|pair| pair[0] == pair[1]),
            "all threads must observe the same decoded profile: {addresses:?}"
        );
    }
}
