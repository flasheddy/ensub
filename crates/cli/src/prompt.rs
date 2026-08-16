use anyhow::{Context, Result};
use console::Term;
use core_engine::ReviewRating;
use dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select};

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
}

impl TerminalPrompt {
    pub fn stderr() -> Self {
        Self {
            terminal: Term::stderr(),
        }
    }
}

impl Prompt for TerminalPrompt {
    fn is_interactive(&self) -> bool {
        self.terminal.is_term()
    }

    fn select_candidates(&mut self, labels: &[String]) -> Result<Vec<usize>> {
        MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select words to capture")
            .items(labels)
            .interact_on(&self.terminal)
            .context("candidate selection failed")
    }

    fn wait_for_reveal(&mut self) -> Result<()> {
        let _: String = Input::with_theme(&ColorfulTheme::default())
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
        let selection = Select::with_theme(&ColorfulTheme::default())
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
