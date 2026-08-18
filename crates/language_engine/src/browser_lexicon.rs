use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Lexicon, LexiconEntry};

const MAGIC: &[u8; 8] = b"ESBLX\0\r\n";
pub const BROWSER_LEXICON_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserLexiconForm {
    pub surface: String,
    pub entry_index: u32,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserLexiconAsset {
    pub schema_version: u16,
    pub definition_source: String,
    pub pronunciation_source: String,
    pub entries: Vec<LexiconEntry>,
    pub forms: Vec<BrowserLexiconForm>,
}

impl BrowserLexiconAsset {
    pub fn encode(&self) -> Result<Vec<u8>, BrowserLexiconError> {
        validate(self)?;
        self.encode_unchecked()
    }

    pub fn encode_unchecked(&self) -> Result<Vec<u8>, BrowserLexiconError> {
        let payload = postcard::to_allocvec(self).map_err(BrowserLexiconError::Encode)?;
        let mut encoded = Vec::with_capacity(MAGIC.len().saturating_add(payload.len()));
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&payload);
        Ok(encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserLexicon {
    asset: BrowserLexiconAsset,
}

impl BrowserLexicon {
    pub fn decode(bytes: &[u8]) -> Result<Self, BrowserLexiconError> {
        let payload = bytes
            .strip_prefix(MAGIC)
            .ok_or(BrowserLexiconError::InvalidMagic)?;
        let asset: BrowserLexiconAsset = postcard::from_bytes(payload)?;
        validate(&asset)?;
        Ok(Self { asset })
    }

    pub fn entry_count(&self) -> usize {
        self.asset.entries.len()
    }

    pub fn form_count(&self) -> usize {
        self.asset.forms.len()
    }

    pub fn definition_source(&self) -> &str {
        &self.asset.definition_source
    }

    pub fn pronunciation_source(&self) -> &str {
        &self.asset.pronunciation_source
    }

    pub fn lookup_entries(&self, surface: &str) -> Vec<LexiconEntry> {
        let normalized = surface.trim().to_lowercase();
        let start = self
            .asset
            .forms
            .partition_point(|form| form.surface.as_str() < normalized.as_str());
        let mut entries = Vec::new();
        for form in self.asset.forms[start..]
            .iter()
            .take_while(|form| form.surface == normalized)
        {
            let Some(entry) = usize::try_from(form.entry_index)
                .ok()
                .and_then(|index| self.asset.entries.get(index))
            else {
                continue;
            };
            if !entries
                .iter()
                .any(|existing: &LexiconEntry| existing.lemma == entry.lemma)
            {
                entries.push(entry.clone());
            }
        }
        entries
    }
}

impl Lexicon for BrowserLexicon {
    type Error = Infallible;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error> {
        Ok(self.lookup_entries(surface).into_iter().next())
    }
}

#[derive(Debug, Error)]
pub enum BrowserLexiconError {
    #[error("browser lexicon has an invalid file signature")]
    InvalidMagic,

    #[error("browser lexicon schema {actual} is unsupported; expected {expected}")]
    UnsupportedSchema { expected: u16, actual: u16 },

    #[error("browser lexicon metadata is incomplete")]
    MissingMetadata,

    #[error("browser lexicon entries must be ordered by lemma")]
    EntriesNotSorted,

    #[error("browser lexicon entry {index} is incomplete")]
    InvalidEntry { index: usize },

    #[error("browser lexicon forms must be strictly ordered by normalized surface")]
    FormsNotSorted,

    #[error("browser lexicon form {form_index} references missing entry {entry_index}")]
    InvalidEntryIndex { form_index: usize, entry_index: u32 },

    #[error("browser lexicon could not be encoded: {0}")]
    Encode(postcard::Error),

    #[error("browser lexicon could not be decoded: {0}")]
    Decode(postcard::Error),
}

fn validate(asset: &BrowserLexiconAsset) -> Result<(), BrowserLexiconError> {
    if asset.schema_version != BROWSER_LEXICON_SCHEMA_VERSION {
        return Err(BrowserLexiconError::UnsupportedSchema {
            expected: BROWSER_LEXICON_SCHEMA_VERSION,
            actual: asset.schema_version,
        });
    }
    if asset.definition_source.trim().is_empty() || asset.pronunciation_source.trim().is_empty() {
        return Err(BrowserLexiconError::MissingMetadata);
    }
    if !asset
        .entries
        .windows(2)
        .all(|pair| pair[0].lemma < pair[1].lemma)
    {
        return Err(BrowserLexiconError::EntriesNotSorted);
    }
    for (index, entry) in asset.entries.iter().enumerate() {
        if entry.lemma.trim().is_empty()
            || entry.phonetic.trim().is_empty()
            || entry.definitions.is_empty()
            || entry.definitions.iter().any(|definition| {
                definition.part_of_speech.trim().is_empty() || definition.text.trim().is_empty()
            })
        {
            return Err(BrowserLexiconError::InvalidEntry { index });
        }
    }
    if !asset.forms.windows(2).all(|pair| {
        (&pair[0].surface, pair[0].priority, pair[0].entry_index)
            < (&pair[1].surface, pair[1].priority, pair[1].entry_index)
            && pair[0].surface == pair[0].surface.trim().to_lowercase()
    }) || asset
        .forms
        .last()
        .is_some_and(|form| form.surface != form.surface.trim().to_lowercase())
    {
        return Err(BrowserLexiconError::FormsNotSorted);
    }
    for (form_index, form) in asset.forms.iter().enumerate() {
        let entry_index = usize::try_from(form.entry_index).map_err(|_| {
            BrowserLexiconError::InvalidEntryIndex {
                form_index,
                entry_index: form.entry_index,
            }
        })?;
        if entry_index >= asset.entries.len() {
            return Err(BrowserLexiconError::InvalidEntryIndex {
                form_index,
                entry_index: form.entry_index,
            });
        }
    }
    Ok(())
}

impl From<postcard::Error> for BrowserLexiconError {
    fn from(error: postcard::Error) -> Self {
        BrowserLexiconError::Decode(error)
    }
}
