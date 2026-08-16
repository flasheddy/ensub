use std::process::Command;

#[test]
fn usage_exposes_reproducible_browser_export_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_ensub-lexicon-builder"))
        .output()
        .expect("builder binary must execute");
    let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");

    assert!(!output.status.success());
    assert!(stderr.contains("export-browser <input.sqlite3.zst> <output.postcard.gz>"));
}
