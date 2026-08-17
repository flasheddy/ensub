use std::process::Command;

#[test]
fn exporter_prints_the_default_stylesheet() {
    let output = Command::new(env!("CARGO_BIN_EXE_ensub-theme-css"))
        .output()
        .expect("theme exporter must start");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("exporter output must be UTF-8"),
        ensub_theme::Theme::default().to_css()
    );
    assert!(output.stderr.is_empty());
}
