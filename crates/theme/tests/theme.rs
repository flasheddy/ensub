use ensub_theme::{ColorScheme, Rgb, Theme};

#[test]
fn default_is_catppuccin_mocha_mauve() {
    let theme = Theme::default();

    assert_eq!(theme, Theme::CATPPUCCIN_MOCHA_MAUVE);
    assert_eq!(theme.scheme, ColorScheme::Dark);
    assert_eq!(theme.background, Rgb::new(0x1e, 0x1e, 0x2e));
    assert_eq!(theme.surface, Rgb::new(0x18, 0x18, 0x25));
    assert_eq!(theme.surface_raised, Rgb::new(0x31, 0x32, 0x44));
    assert_eq!(theme.surface_overlay, Rgb::new(0x45, 0x47, 0x5a));
    assert_eq!(theme.border, Rgb::new(0x45, 0x47, 0x5a));
    assert_eq!(theme.border_strong, Rgb::new(0x58, 0x5b, 0x70));
    assert_eq!(theme.text, Rgb::new(0xcd, 0xd6, 0xf4));
    assert_eq!(theme.text_muted, Rgb::new(0xba, 0xc2, 0xde));
    assert_eq!(theme.text_subtle, Rgb::new(0xa6, 0xad, 0xc8));
    assert_eq!(theme.accent, Rgb::new(0xcb, 0xa6, 0xf7));
    assert_eq!(theme.on_accent, Rgb::new(0x11, 0x11, 0x1b));
    assert_eq!(theme.focus, theme.accent);
    assert_eq!(theme.selection, theme.accent);
    assert_eq!(theme.on_selection, theme.on_accent);
    assert_eq!(theme.success, Rgb::new(0xa6, 0xe3, 0xa1));
    assert_eq!(theme.warning, Rgb::new(0xf9, 0xe2, 0xaf));
    assert_eq!(theme.danger, Rgb::new(0xf3, 0x8b, 0xa8));
    assert_eq!(theme.info, Rgb::new(0x89, 0xb4, 0xfa));
}

#[test]
fn semantic_roles_can_be_overridden_with_struct_update_syntax() {
    let custom = Theme {
        accent: Rgb::new(1, 2, 3),
        focus: Rgb::new(4, 5, 6),
        ..Theme::default()
    };

    assert_eq!(custom.accent, Rgb::new(1, 2, 3));
    assert_eq!(custom.focus, Rgb::new(4, 5, 6));
    assert_eq!(custom.background, Theme::default().background);
}

#[test]
fn rgb_formats_as_lowercase_css_hex() {
    assert_eq!(Rgb::new(0x0a, 0xbc, 0x01).to_hex(), "#0abc01");
}

#[test]
fn css_output_is_stable_and_complete() {
    let css = Theme::default().to_css();

    assert!(css.starts_with(":root {\n  color-scheme: dark;\n"));
    assert!(css.contains("  --ensub-color-background: #1e1e2e;\n"));
    assert!(css.contains("  --ensub-color-accent: #cba6f7;\n"));
    assert!(css.contains("  --ensub-color-on-selection: #11111b;\n"));
    assert!(css.contains("  --ensub-color-danger: #f38ba8;\n"));
    assert!(css.ends_with("}\n"));

    for role in [
        "background",
        "surface",
        "surface-raised",
        "surface-overlay",
        "border",
        "border-strong",
        "text",
        "text-muted",
        "text-subtle",
        "accent",
        "on-accent",
        "focus",
        "selection",
        "on-selection",
        "success",
        "warning",
        "danger",
        "info",
    ] {
        assert_eq!(css.matches(&format!("--ensub-color-{role}:")).count(), 1);
    }
}

#[test]
fn default_reading_pairs_meet_wcag_aa_contrast() {
    let theme = Theme::default();

    for (foreground, background) in [
        (theme.text, theme.background),
        (theme.text_muted, theme.background),
        (theme.text_subtle, theme.background),
        (theme.on_accent, theme.accent),
        (theme.on_selection, theme.selection),
        (theme.success, theme.background),
        (theme.warning, theme.background),
        (theme.danger, theme.background),
        (theme.info, theme.background),
    ] {
        assert!(contrast_ratio(foreground, background) >= 4.5);
    }
}

fn contrast_ratio(first: Rgb, second: Rgb) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn luminance(color: Rgb) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    (0.2126 * channel(color.red)) + (0.7152 * channel(color.green)) + (0.0722 * channel(color.blue))
}
