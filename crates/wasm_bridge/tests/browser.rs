#![cfg(target_arch = "wasm32")]

use js_sys::Uint8Array;
use language_engine::{
    BrowserLexiconAsset, BrowserLexiconForm, Definition, LexiconEntry,
    BROWSER_LEXICON_SCHEMA_VERSION,
};
use wasm_bindgen_test::*;

use ensub_wasm::{
    CaptureParsedInput, EnsubSandbox, ParseInput, ParseOutput, StatsInput, StatsOutput,
};

wasm_bindgen_test_configure!(run_in_browser);

fn lexicon_bytes() -> Vec<u8> {
    BrowserLexiconAsset {
        schema_version: BROWSER_LEXICON_SCHEMA_VERSION,
        definition_source: "browser test".to_string(),
        pronunciation_source: "browser test".to_string(),
        entries: vec![LexiconEntry {
            lemma: "immersion".to_string(),
            phonetic: "ɪˈmɜːʃən".to_string(),
            definitions: vec![Definition {
                part_of_speech: "noun".to_string(),
                text: "deep involvement".to_string(),
            }],
        }],
        forms: vec![BrowserLexiconForm {
            surface: "immersion".to_string(),
            entry_index: 0,
            priority: 0,
        }],
    }
    .encode()
    .expect("fixture lexicon must encode")
}

#[wasm_bindgen_test]
fn exported_sandbox_round_trips_javascript_dtos_through_local_storage() {
    let key = "ensub.wasm-browser-test.v1";
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .expect("browser test requires localStorage");
    storage.remove_item(key).expect("fixture must reset");
    let bytes = lexicon_bytes();
    let mut sandbox = EnsubSandbox::new(Uint8Array::from(bytes.as_slice()), key.to_string(), false)
        .expect("sandbox must open");
    let parsed = sandbox
        .parse(
            serde_wasm_bindgen::to_value(&ParseInput {
                text: "Immersion works.".to_string(),
                include_stopwords: false,
                max_candidates: 10,
            })
            .expect("parse input must serialize"),
        )
        .expect("text must parse");
    let parsed: ParseOutput =
        serde_wasm_bindgen::from_value(parsed).expect("parse output must deserialize");
    let candidate_id = parsed.candidates[0].id.clone();
    sandbox
        .capture_parsed(
            serde_wasm_bindgen::to_value(&CaptureParsedInput {
                text: "Immersion works.".to_string(),
                candidate_ids: vec![candidate_id],
                source: "wasm:test".to_string(),
                captured_at_ms: 1_755_244_800_000_i64,
                include_stopwords: false,
                max_candidates: 10,
            })
            .expect("capture input must serialize"),
        )
        .expect("candidate must save");

    let reopened = EnsubSandbox::new(Uint8Array::from(bytes.as_slice()), key.to_string(), true)
        .expect("read-only sandbox must reopen");
    let stats = reopened
        .stats(
            serde_wasm_bindgen::to_value(&StatsInput {
                as_of_ms: 1_755_244_800_000_i64,
            })
            .expect("stats input must serialize"),
        )
        .expect("stats must load");
    let stats: StatsOutput =
        serde_wasm_bindgen::from_value(stats).expect("stats output must deserialize");
    assert_eq!(stats.total_cards, 1);
    assert_eq!(stats.due_cards, 1);
    storage.remove_item(key).expect("fixture must clean up");
}
