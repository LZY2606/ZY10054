//! Regenerates `tests/regression_snapshot.json`, the guard snapshot used by the
//! regression tests to prove that detection results do not change.
//!
//! Run from the repository root:
//!
//! ```sh
//! cargo run --example regenerate_snapshot --features dev
//! ```
//!
//! The output is deterministic: samples are emitted in a fixed order and
//! floating point values use the shortest round-trip representation.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use whatlang::dev::{self, FilterList};
use whatlang::{Lang, Options, detect_with_options};

fn parse_langs(codes: &[String]) -> Vec<Lang> {
    codes
        .iter()
        .map(|code| Lang::from_code(code).unwrap_or_else(|| panic!("unknown lang code: {code}")))
        .collect()
}

fn filter_list(allow: &Option<Vec<String>>, deny: &Option<Vec<String>>) -> FilterList {
    match (allow, deny) {
        (Some(codes), None) => FilterList::allow(parse_langs(codes)),
        (None, Some(codes)) => FilterList::deny(parse_langs(codes)),
        (None, None) => FilterList::All,
        _ => panic!("allow and deny are mutually exclusive"),
    }
}

fn fmt_f64(value: f64) -> String {
    // serde_json uses the shortest round-trip representation for f64,
    // so formatting via serde_json keeps the snapshot exact and deterministic.
    serde_json::to_string(&value).unwrap()
}

fn snapshot_entry(name: &str, text: &str, filter_list: &FilterList, out: &mut String) {
    let options = Options::new().set_filter_list(filter_list.clone());
    let info = detect_with_options(text, &options);
    let scores = dev::raw_trigram_scores(text, filter_list);

    let _ = writeln!(out, "    {{");
    let _ = writeln!(out, "      \"name\": {},", serde_json::to_string(name).unwrap());
    match info {
        Some(info) => {
            let _ = writeln!(
                out,
                "      \"detect\": {{ \"lang\": {:?}, \"script\": {:?}, \"confidence\": {} }},",
                info.lang().code(),
                info.script().name(),
                fmt_f64(info.confidence())
            );
        }
        None => {
            let _ = writeln!(out, "      \"detect\": null,");
        }
    }
    match scores {
        Some(scores) => {
            let pairs: Vec<String> = scores
                .iter()
                .map(|(lang, score)| format!("[{:?}, {}]", lang.code(), fmt_f64(*score)))
                .collect();
            let _ = writeln!(out, "      \"trigram_scores\": [{}]", pairs.join(", "));
        }
        None => {
            let _ = writeln!(out, "      \"trigram_scores\": null");
        }
    }
    let _ = write!(out, "    }}");
}

fn opt_string_list(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    value.get(key).map(|list| {
        list.as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap().to_owned())
            .collect()
    })
}

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let examples_path = format!("{manifest_dir}/tests/examples.json");
    let boundary_path = format!("{manifest_dir}/tests/boundary_samples.json");
    let snapshot_path = format!("{manifest_dir}/tests/regression_snapshot.json");

    let examples: BTreeMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(&examples_path).unwrap()).unwrap();
    let boundary: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(&boundary_path).unwrap()).unwrap();

    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"version\": 1,");
    let _ = writeln!(out, "  \"samples\": [");

    let mut entries: Vec<String> = Vec::new();

    for (lang_code, text) in &examples {
        let name = format!("corpus:{lang_code}");
        let mut entry = String::new();
        snapshot_entry(&name, text, &FilterList::All, &mut entry);
        entries.push(entry);
    }
    for sample in &boundary {
        let name = format!("boundary:{}", sample["name"].as_str().unwrap());
        let text = sample["text"].as_str().unwrap();
        let allow = opt_string_list(sample, "allow");
        let deny = opt_string_list(sample, "deny");
        let filter_list = filter_list(&allow, &deny);
        let mut entry = String::new();
        snapshot_entry(&name, text, &filter_list, &mut entry);
        entries.push(entry);
    }

    let _ = writeln!(out, "{}", entries.join(",\n"));
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "}}");

    std::fs::write(&snapshot_path, out).unwrap();
    println!("wrote {snapshot_path}");
}
