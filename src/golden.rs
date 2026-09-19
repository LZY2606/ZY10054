//! Golden-master guard for detection results.
//!
//! The expected outputs (language, script, confidence and full candidate
//! rankings) for the corpus in `tests/examples.json` and the boundary
//! samples in `tests/boundary_samples.json` are stored in
//! `tests/golden_detect.json`. The guard fails if any of them change,
//! e.g. because of a profile storage refactor.
//!
//! To regenerate the golden file (only when a behavior change is intended):
//!
//! ```text
//! cargo test generate_golden -- --ignored
//! ```

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::core::{FilterList, InternalQuery, Options, Text, detect_with_options};
    use crate::scripts::grouping::ScriptLangGroup;
    use crate::scripts::{Script, raw_detect_script};
    use crate::{Lang, combined, trigrams};

    const GOLDEN_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden_detect.json");

    // `cargo test --quiet` hides test names; announce the validation stage
    // on stderr (which is not captured) so it stays visible.
    fn announce_stage() {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr(),
            "stage: golden::verify_golden (lang/script/confidence/rankings vs tests/golden_detect.json)"
        );
    }

    struct Sample {
        id: String,
        text: String,
        filter_list: FilterList,
    }

    fn script_by_name(name: &str) -> Script {
        *Script::all()
            .iter()
            .find(|script| script.name() == name)
            .unwrap_or_else(|| panic!("unknown script name: {name}"))
    }

    fn load_samples() -> Vec<Sample> {
        let mut samples = Vec::new();

        let examples: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../tests/examples.json")).unwrap();
        for (code, text) in examples {
            samples.push(Sample {
                id: format!("example:{code}"),
                text,
                filter_list: FilterList::All,
            });
        }

        let boundary: Value =
            serde_json::from_str(include_str!("../tests/boundary_samples.json")).unwrap();
        for item in boundary.as_array().unwrap() {
            let id = item["id"].as_str().unwrap().to_string();
            let text = item["text"].as_str().unwrap().to_string();
            let filter_list = if let Some(allow) = item.get("allow") {
                let langs = allow
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|code| Lang::from_code(code.as_str().unwrap()).unwrap())
                    .collect();
                FilterList::allow(langs)
            } else if let Some(script_name) = item.get("deny_script") {
                let script = script_by_name(script_name.as_str().unwrap());
                FilterList::deny(script.langs().to_owned())
            } else {
                FilterList::All
            };
            samples.push(Sample {
                id: format!("boundary:{id}"),
                text,
                filter_list,
            });
        }

        samples
    }

    // Candidate rankings for the multi-language-script code path.
    // Returns (trigram ranking, combined ranking) as JSON values.
    fn rankings(text: &str, filter_list: &FilterList) -> (Value, Value) {
        let raw_script_info = raw_detect_script(text);
        let Some(script) = raw_script_info.main_script() else {
            return (Value::Null, Value::Null);
        };
        let ScriptLangGroup::Multi(multi_lang_script) = script.to_lang_group() else {
            return (Value::Null, Value::Null);
        };

        let iquery = InternalQuery {
            text: Text::new(text),
            filter_list,
            multi_lang_script,
        };
        let trigram_outcome = trigrams::raw_detect(&iquery);
        let combined_outcome = combined::raw_detect(&iquery);

        let trigram_ranking: Vec<Value> = trigram_outcome
            .raw_distances
            .iter()
            .map(|(lang, distance)| json!([lang.code(), distance]))
            .collect();
        let combined_ranking: Vec<Value> = combined_outcome
            .scores
            .iter()
            .map(|(lang, score)| json!([lang.code(), score.to_bits()]))
            .collect();
        (json!(trigram_ranking), json!(combined_ranking))
    }

    fn expectations(sample: &Sample) -> Value {
        let options = Options::new().set_filter_list(sample.filter_list.clone());
        let info = detect_with_options(&sample.text, &options);
        let (trigram_ranking, combined_ranking) = rankings(&sample.text, &sample.filter_list);
        match info {
            Some(info) => json!({
                "lang": info.lang().code(),
                "script": info.script().name(),
                "confidence_bits": info.confidence().to_bits(),
                "trigram_ranking": trigram_ranking,
                "combined_ranking": combined_ranking,
            }),
            None => json!({
                "lang": null,
                "script": null,
                "confidence_bits": null,
                "trigram_ranking": trigram_ranking,
                "combined_ranking": combined_ranking,
            }),
        }
    }

    #[test]
    #[ignore]
    fn generate_golden() {
        let mut golden = serde_json::Map::new();
        for sample in load_samples() {
            golden.insert(sample.id.clone(), expectations(&sample));
        }
        let doc = json!({
            "format": 1,
            "samples": golden,
        });
        let json = serde_json::to_string_pretty(&doc).unwrap();
        std::fs::write(GOLDEN_PATH, format!("{json}\n")).unwrap();
        println!("wrote {GOLDEN_PATH}");
    }

    #[test]
    fn verify_golden() {
        announce_stage();
        let golden: Value = serde_json::from_str(include_str!("../tests/golden_detect.json"))
            .expect("tests/golden_detect.json must exist and be valid JSON");
        assert_eq!(golden["format"], json!(1), "unsupported golden format");
        let golden_samples = golden["samples"].as_object().unwrap();

        let samples = load_samples();
        let mut failures = Vec::new();
        for sample in &samples {
            let actual = expectations(sample);
            match golden_samples.get(&sample.id) {
                Some(expected) => {
                    if *expected != actual {
                        failures.push(format!(
                            "{}:\n  expected: {}\n  actual:   {}",
                            sample.id, expected, actual
                        ));
                    }
                }
                None => failures.push(format!("{}: missing in golden file", sample.id)),
            }
        }
        for id in golden_samples.keys() {
            if !samples.iter().any(|sample| &sample.id == id) {
                failures.push(format!("{id}: stale entry in golden file"));
            }
        }
        assert!(
            failures.is_empty(),
            "golden master mismatch ({} sample(s)):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
