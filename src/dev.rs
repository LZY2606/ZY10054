//! This mod exposes some internal API.
//! It exists only to enable tuning of the library with extra supporting tools (e.g. benchmarks).
//! Developers are advised against relying on API.
//!
pub use crate::alphabets::{RawOutcome as RawAlphabetsInfo, raw_detect as alphabets_raw_detect};
pub use crate::combined::{RawOutcome as RawCombinedInfo, raw_detect as combined_raw_detect};
pub use crate::core::{Detector, Info, Method, Options, detect, detect_lang, detect_with_options};
pub use crate::lang::Lang;
pub use crate::scripts::{RawScriptInfo, Script, detect_script, raw_detect_script};
pub use crate::trigrams::{RawOutcome as RawTrigramsInfo, raw_detect as trigrams_raw_detect};

pub use crate::alphabets::cyrillic::alphabet_calculate_scores as alphabet_cyrillic_calculate_scores;
pub use crate::alphabets::latin::alphabet_calculate_scores as alphabet_latin_calculate_scores;
pub use crate::core::{FilterList, LowercaseText};

// private imports
use crate::core::Query;
use crate::core::detect::detect_lang_base_on_mandarin_script;
use crate::scripts::grouping::ScriptLangGroup;

#[derive(Debug)]
pub struct RawInfo {
    pub script_info: RawScriptInfo,
    pub lang_info: Option<RawLangInfo>,
}

#[derive(Debug)]
pub enum RawLangInfo {
    OneScript(Lang),
    MultiScript(RawCombinedInfo),
    Mandarin(Lang),
}

pub fn raw_detect(text: &str) -> RawInfo {
    let script_info = raw_detect_script(text);

    let query = Query {
        text,
        filter_list: &FilterList::default(),
        method: Method::Combined,
    };

    let lang_info = script_info
        .main_script()
        .map(|script| match script.to_lang_group() {
            ScriptLangGroup::One(lang) => RawLangInfo::OneScript(lang),
            ScriptLangGroup::Multi(multi_lang_script) => {
                let iquery = query.to_internal(multi_lang_script);
                let combined = combined_raw_detect(&iquery);
                RawLangInfo::MultiScript(combined)
            }
            ScriptLangGroup::Mandarin => {
                let lang = detect_lang_base_on_mandarin_script(&query, &script_info).lang();
                RawLangInfo::Mandarin(lang)
            }
        });

    RawInfo {
        script_info,
        lang_info,
    }
}

/// Computes the full trigram candidate scores for a text with the given filter
/// list, exactly as the trigram detection method produces them.
/// Returns `None` when the text's script does not need trigram-based detection.
/// It exists to support regression snapshots and profiling tools.
pub fn raw_trigram_scores(text: &str, filter_list: &FilterList) -> Option<Vec<(Lang, f64)>> {
    let raw_script_info = raw_detect_script(text);
    let script = raw_script_info.main_script()?;
    match script.to_lang_group() {
        ScriptLangGroup::Multi(multi_lang_script) => {
            let query = Query {
                text,
                filter_list,
                method: Method::Trigram,
            };
            let iquery = query.to_internal(multi_lang_script);
            Some(crate::trigrams::raw_detect(&iquery).scores)
        }
        _ => None,
    }
}

/// Number of bytes the embedded trigram profiles occupy in the binary.
pub fn profile_storage_bytes() -> usize {
    use crate::trigrams::{ARABIC_LANGS, CYRILLIC_LANGS, DEVANAGARI_LANGS, HEBREW_LANGS, LATIN_LANGS};
    let mut total = 0;
    for list in [
        LATIN_LANGS,
        CYRILLIC_LANGS,
        ARABIC_LANGS,
        DEVANAGARI_LANGS,
        HEBREW_LANGS,
    ] {
        total += std::mem::size_of_val(list);
        for &(_, profile) in list {
            total += std::mem::size_of_val(profile);
        }
    }
    total
}

/// Number of heap bytes currently used by lazily decoded trigram profiles.
/// The eager profile layout keeps everything in static memory, so this is 0.
pub fn decoded_profile_bytes() -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_detect() {
        let zapovit = r#"
            Як умру, то поховайте
            Мене на могилі,
            Серед степу широкого,
            На Вкраїні милій,
            Щоб лани широкополі,
            І Дніпро, і кручі
            Було видно, було чути,
            Як реве ревучий.
        "#;
        let info = raw_detect(&zapovit);
        assert_eq!(info.script_info.counters[0].0, Script::Cyrillic);
    }
}
