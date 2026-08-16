use language_engine::lemma_candidates;

#[test]
fn irregular_forms_put_the_canonical_lemma_first() {
    assert_eq!(lemma_candidates("went")[0], "go");
    assert_eq!(lemma_candidates("mice")[0], "mouse");
    assert_eq!(lemma_candidates("better")[0], "good");
}

#[test]
fn regular_inflections_include_plausible_canonical_lemmas() {
    assert!(lemma_candidates("immersed").contains(&"immerse".to_string()));
    assert!(lemma_candidates("studies").contains(&"study".to_string()));
    assert!(lemma_candidates("running").contains(&"run".to_string()));
}

#[test]
fn candidates_are_lowercase_unique_and_include_the_surface() {
    let candidates = lemma_candidates("WALKED");

    assert_eq!(candidates[0], "walked");
    assert!(candidates.contains(&"walk".to_string()));
    let mut deduplicated = candidates.clone();
    deduplicated.sort();
    deduplicated.dedup();
    assert_eq!(deduplicated.len(), candidates.len());
}
