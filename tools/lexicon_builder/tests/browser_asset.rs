use std::io::Read;

use flate2::read::GzDecoder;
use language_engine::{BrowserLexicon, Lexicon};
use sha2::Digest;

#[test]
fn tracked_browser_asset_has_full_pinned_corpus() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/web_sandbox/assets/lexicon-v1.postcard.gz");
    let compressed = std::fs::read(&path).expect("tracked browser lexicon must read");
    let mut decoded = Vec::new();
    GzDecoder::new(compressed.as_slice())
        .read_to_end(&mut decoded)
        .expect("tracked browser lexicon must decompress");
    let lexicon = BrowserLexicon::decode(&decoded).expect("tracked browser lexicon must decode");
    let went = lexicon
        .lookup("went")
        .expect("browser lookup must execute")
        .expect("full corpus must include went");

    assert_eq!(lexicon.entry_count(), 32_463);
    assert_eq!(lexicon.form_count(), 49_207);
    assert_eq!(went.lemma, "go");
    assert!(!went.phonetic.is_empty());
    assert!(!went.definitions.is_empty());
}

#[test]
fn tracked_manifest_matches_browser_asset_bytes_and_sources() {
    let assets =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/web_sandbox/assets");
    let compressed = std::fs::read(assets.join("lexicon-v1.postcard.gz"))
        .expect("tracked browser lexicon must read");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(assets.join("lexicon-v1.manifest.json"))
            .expect("tracked lexicon manifest must read"),
    )
    .expect("tracked lexicon manifest must parse");
    let digest = sha2::Sha256::digest(&compressed)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["lexemeCount"], 32_463);
    assert_eq!(manifest["formCount"], 49_207);
    assert_eq!(manifest["compressedBytes"], compressed.len());
    assert_eq!(manifest["compressedSha256"], digest);
    assert_eq!(manifest["definitionSource"], "Open English WordNet 2025");
    assert_eq!(
        manifest["pronunciationSource"],
        "CMUdict 0.7b via cmudict-fast 0.8.0"
    );
}
