//! Portable semantic color themes for Ensub frontends.

#![forbid(unsafe_code)]

/// An opaque 24-bit sRGB color shared across frontend adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

/// The preferred native control and browser color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorScheme {
    Dark,
    Light,
}

impl ColorScheme {
    const fn css_value(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// A complete set of semantic colors independent of any UI toolkit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Theme {
    pub scheme: ColorScheme,
    pub background: Rgb,
    pub surface: Rgb,
    pub surface_raised: Rgb,
    pub surface_overlay: Rgb,
    pub border: Rgb,
    pub border_strong: Rgb,
    pub text: Rgb,
    pub text_muted: Rgb,
    pub text_subtle: Rgb,
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub focus: Rgb,
    pub selection: Rgb,
    pub on_selection: Rgb,
    pub success: Rgb,
    pub warning: Rgb,
    pub danger: Rgb,
    pub info: Rgb,
}

impl Theme {
    /// Catppuccin Mocha with Mauve as its accent.
    pub const CATPPUCCIN_MOCHA_MAUVE: Self = Self {
        scheme: ColorScheme::Dark,
        background: Rgb::new(0x1e, 0x1e, 0x2e),
        surface: Rgb::new(0x18, 0x18, 0x25),
        surface_raised: Rgb::new(0x31, 0x32, 0x44),
        surface_overlay: Rgb::new(0x45, 0x47, 0x5a),
        border: Rgb::new(0x45, 0x47, 0x5a),
        border_strong: Rgb::new(0x58, 0x5b, 0x70),
        text: Rgb::new(0xcd, 0xd6, 0xf4),
        text_muted: Rgb::new(0xba, 0xc2, 0xde),
        text_subtle: Rgb::new(0xa6, 0xad, 0xc8),
        accent: Rgb::new(0xcb, 0xa6, 0xf7),
        on_accent: Rgb::new(0x11, 0x11, 0x1b),
        focus: Rgb::new(0xcb, 0xa6, 0xf7),
        selection: Rgb::new(0xcb, 0xa6, 0xf7),
        on_selection: Rgb::new(0x11, 0x11, 0x1b),
        success: Rgb::new(0xa6, 0xe3, 0xa1),
        warning: Rgb::new(0xf9, 0xe2, 0xaf),
        danger: Rgb::new(0xf3, 0x8b, 0xa8),
        info: Rgb::new(0x89, 0xb4, 0xfa),
    };

    /// Render this theme as a deterministic browser custom-property sheet.
    #[must_use]
    pub fn to_css(self) -> String {
        let mut css = format!(":root {{\n  color-scheme: {};\n", self.scheme.css_value());
        for (name, color) in [
            ("background", self.background),
            ("surface", self.surface),
            ("surface-raised", self.surface_raised),
            ("surface-overlay", self.surface_overlay),
            ("border", self.border),
            ("border-strong", self.border_strong),
            ("text", self.text),
            ("text-muted", self.text_muted),
            ("text-subtle", self.text_subtle),
            ("accent", self.accent),
            ("on-accent", self.on_accent),
            ("focus", self.focus),
            ("selection", self.selection),
            ("on-selection", self.on_selection),
            ("success", self.success),
            ("warning", self.warning),
            ("danger", self.danger),
            ("info", self.info),
        ] {
            css.push_str("  --ensub-color-");
            css.push_str(name);
            css.push_str(": ");
            css.push_str(&color.to_hex());
            css.push_str(";\n");
        }
        css.push_str("}\n");
        css
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::CATPPUCCIN_MOCHA_MAUVE
    }
}
