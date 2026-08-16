use std::path::PathBuf;

use ensub_sqlite::{lexicon_cache_dir, resolve_database_path};

#[test]
fn explicit_database_path_is_preserved() {
    let requested = PathBuf::from("custom/ensub.sqlite3");

    let resolved = resolve_database_path(Some(requested.clone()))
        .expect("explicit database path must resolve");

    assert_eq!(resolved, requested);
}

#[test]
fn platform_paths_use_stable_ensub_suffixes() {
    let database = resolve_database_path(None).expect("platform data directory must resolve");
    let lexicon = lexicon_cache_dir().expect("platform cache directory must resolve");

    assert!(database.ends_with(PathBuf::from("ensub").join("ensub.sqlite3")));
    assert!(lexicon.ends_with(PathBuf::from("ensub").join("lexicon")));
}
