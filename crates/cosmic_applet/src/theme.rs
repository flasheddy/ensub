use std::sync::Arc;

use cosmic::cosmic_theme::palette::{Srgb, Srgba};
use cosmic::cosmic_theme::ThemeBuilder;
use ensub_theme::{ColorScheme, Rgb, Theme};

pub fn to_cosmic_theme(theme: Theme) -> cosmic::Theme {
    let mut builder = match theme.scheme {
        ColorScheme::Dark => ThemeBuilder::dark(),
        ColorScheme::Light => ThemeBuilder::light(),
    }
    .bg_color(srgba(theme.background))
    .primary_container_bg(srgba(theme.surface_raised))
    .neutral_tint(srgb(theme.surface_overlay))
    .text_tint(srgb(theme.text))
    .accent(srgb(theme.accent))
    .success(srgb(theme.success))
    .warning(srgb(theme.warning))
    .destructive(srgb(theme.danger));
    builder.secondary_container_bg = Some(srgba(theme.surface));
    builder.window_hint = Some(srgb(theme.focus));

    let mut palette = builder.build();
    palette.name = "Ensub".to_string();
    palette.accent.on = srgba(theme.on_accent);
    palette.accent.selected = srgba(theme.selection);
    palette.accent.selected_text = srgba(theme.on_selection);
    palette.accent.focus = srgba(theme.focus);
    palette.accent_button.on = srgba(theme.on_accent);
    palette.accent_button.selected = srgba(theme.selection);
    palette.accent_button.selected_text = srgba(theme.on_selection);
    palette.accent_button.focus = srgba(theme.focus);
    palette.button.selected = srgba(theme.selection);
    palette.button.selected_text = srgba(theme.on_selection);
    palette.button.focus = srgba(theme.focus);
    palette.icon_button.focus = srgba(theme.focus);
    palette.list_button.selected = srgba(theme.selection);
    palette.list_button.selected_text = srgba(theme.on_selection);
    palette.list_button.focus = srgba(theme.focus);
    palette.text_button.focus = srgba(theme.focus);

    cosmic::Theme::custom(Arc::new(palette))
}

fn srgb(color: Rgb) -> Srgb {
    Srgb::new(
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
    )
}

fn srgba(color: Rgb) -> Srgba {
    Srgba::new(
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use cosmic::cosmic_theme::palette::Srgba;
    use ensub_theme::{Rgb, Theme};

    use super::to_cosmic_theme;

    #[test]
    fn applet_adapter_uses_shared_accent_and_contrast_color() {
        let theme = Theme {
            accent: Rgb::new(30, 40, 50),
            on_accent: Rgb::new(60, 70, 80),
            ..Theme::default()
        };

        let cosmic = to_cosmic_theme(theme);

        assert_eq!(cosmic.cosmic().accent_color(), srgba(theme.accent));
        assert_eq!(cosmic.cosmic().on_accent_color(), srgba(theme.on_accent));
    }

    fn srgba(color: Rgb) -> Srgba {
        Srgba::new(
            f32::from(color.red) / 255.0,
            f32::from(color.green) / 255.0,
            f32::from(color.blue) / 255.0,
            1.0,
        )
    }
}
