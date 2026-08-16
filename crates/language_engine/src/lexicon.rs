use std::error::Error;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub part_of_speech: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LexiconEntry {
    pub lemma: String,
    pub phonetic: String,
    pub definitions: Vec<Definition>,
}

/// Storage-neutral lookup boundary for native and browser lexicons.
pub trait Lexicon {
    type Error: Error + 'static;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error>;
}
