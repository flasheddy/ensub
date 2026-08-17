use anyhow::{Context, Result};
use console::{style, Color, Style, Term};
use core_engine::ReviewRating;
use dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select};
use ensub_theme::{Rgb, Theme};

pub trait Prompt {
    fn is_interactive(&self) -> bool;

    fn select_candidates(&mut self, labels: &[String]) -> Result<Vec<usize>>;

    fn wait_for_reveal(&mut self) -> Result<()>;

    fn review_response(&mut self) -> Result<ReviewResponse>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewResponse {
    Rating(ReviewRating),
    Quit,
}

pub struct TerminalPrompt {
    terminal: Term,
    theme: ColorfulTheme,
}

impl TerminalPrompt {
    pub fn stderr() -> Self {
        Self::stderr_with_theme(Theme::default())
    }

    pub fn stderr_with_theme(theme: Theme) -> Self {
        Self {
            terminal: Term::stderr(),
            theme: dialoguer_theme(theme),
        }
    }
}

fn dialoguer_theme(theme: Theme) -> ColorfulTheme {
    let accent = terminal_style(theme.accent);
    let success = terminal_style(theme.success);
    let danger = terminal_style(theme.danger);
    let text = terminal_style(theme.text);
    let subtle = terminal_style(theme.text_subtle);

    ColorfulTheme {
        defaults_style: accent.clone(),
        prompt_style: text.clone().bold(),
        prompt_prefix: style("?".to_string())
            .for_stderr()
            .fg(terminal_color(theme.accent)),
        prompt_suffix: style("›".to_string())
            .for_stderr()
            .fg(terminal_color(theme.text_subtle)),
        success_prefix: style("✔".to_string())
            .for_stderr()
            .fg(terminal_color(theme.success)),
        success_suffix: style("·".to_string())
            .for_stderr()
            .fg(terminal_color(theme.text_subtle)),
        error_prefix: style("✘".to_string())
            .for_stderr()
            .fg(terminal_color(theme.danger)),
        error_style: danger,
        hint_style: subtle,
        values_style: success.clone(),
        active_item_style: accent.clone(),
        inactive_item_style: text,
        active_item_prefix: style("❯".to_string())
            .for_stderr()
            .fg(terminal_color(theme.accent)),
        inactive_item_prefix: style(" ".to_string()).for_stderr(),
        checked_item_prefix: style("✔".to_string())
            .for_stderr()
            .fg(terminal_color(theme.success)),
        unchecked_item_prefix: style("⬚".to_string())
            .for_stderr()
            .fg(terminal_color(theme.accent)),
        picked_item_prefix: style("❯".to_string())
            .for_stderr()
            .fg(terminal_color(theme.success)),
        unpicked_item_prefix: style(" ".to_string()).for_stderr(),
    }
}

fn terminal_style(color: Rgb) -> Style {
    Style::new().for_stderr().fg(terminal_color(color))
}

fn terminal_color(color: Rgb) -> Color {
    Color::TrueColor(color.red, color.green, color.blue)
}

impl Prompt for TerminalPrompt {
    fn is_interactive(&self) -> bool {
        self.terminal.is_term()
    }

    fn select_candidates(&mut self, labels: &[String]) -> Result<Vec<usize>> {
        MultiSelect::with_theme(&self.theme)
            .with_prompt("Select words to capture")
            .items(labels)
            .interact_on(&self.terminal)
            .context("candidate selection failed")
    }

    fn wait_for_reveal(&mut self) -> Result<()> {
        let _: String = Input::with_theme(&self.theme)
            .with_prompt("Press Enter to reveal")
            .allow_empty(true)
            .interact_text_on(&self.terminal)
            .context("review reveal prompt failed")?;
        Ok(())
    }

    fn review_response(&mut self) -> Result<ReviewResponse> {
        const ITEMS: [&str; 7] = [
            "0 - complete blackout",
            "1 - incorrect; remembered after reveal",
            "2 - incorrect; familiar answer",
            "3 - correct with serious difficulty",
            "4 - correct with hesitation",
            "5 - perfect recall",
            "quit review",
        ];
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Recall quality")
            .items(ITEMS)
            .default(4)
            .interact_on(&self.terminal)
            .context("review rating prompt failed")?;
        if selection == 6 {
            return Ok(ReviewResponse::Quit);
        }
        let value = u8::try_from(selection).context("review rating is out of range")?;
        Ok(ReviewResponse::Rating(ReviewRating::try_from(value)?))
    }
}

#[cfg(test)]
mod tests {
    use console::{Color, Style};
    use ensub_theme::{Rgb, Theme};

    use super::dialoguer_theme;

    #[test]
    fn dialoguer_styles_follow_semantic_roles() {
        let theme = Theme {
            text: Rgb::new(1, 2, 3),
            text_subtle: Rgb::new(4, 5, 6),
            accent: Rgb::new(7, 8, 9),
            success: Rgb::new(10, 11, 12),
            danger: Rgb::new(13, 14, 15),
            ..Theme::default()
        };

        let dialoguer = dialoguer_theme(theme);

        assert_eq!(
            dialoguer.prompt_style,
            Style::new()
                .for_stderr()
                .fg(Color::TrueColor(1, 2, 3))
                .bold()
        );
        assert_eq!(
            dialoguer.hint_style,
            Style::new().for_stderr().fg(Color::TrueColor(4, 5, 6))
        );
        assert_eq!(
            dialoguer.active_item_style,
            Style::new().for_stderr().fg(Color::TrueColor(7, 8, 9))
        );
        assert_eq!(
            dialoguer.values_style,
            Style::new().for_stderr().fg(Color::TrueColor(10, 11, 12))
        );
        assert_eq!(
            dialoguer.error_style,
            Style::new().for_stderr().fg(Color::TrueColor(13, 14, 15))
        );
    }
}
