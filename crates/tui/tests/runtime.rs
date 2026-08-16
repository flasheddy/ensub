use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, TimeZone, Utc};
use core_engine::{
    Capture, CaptureResult, ContextRecord, ReviewCard, ReviewState, ReviewStatistics, ReviewUpdate,
    StorageAdapter, WordId, WordRecord,
};
use ensub_tui::{execute_effect, Effect, Message, TerminalControl, TerminalGuard};
use language_engine::{Definition, Lexicon, LexiconEntry};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 15, 10, 0, 0)
        .single()
        .expect("test timestamp must be valid")
}

#[derive(Default)]
struct FakeStorage {
    captures: Vec<Capture>,
}

impl StorageAdapter for FakeStorage {
    type Error = Infallible;

    fn save_word(&mut self, _word: &WordRecord) -> Result<(), Self::Error> {
        Ok(())
    }

    fn save_context(&mut self, _context: &ContextRecord) -> Result<(), Self::Error> {
        Ok(())
    }

    fn save_review_state(&mut self, _state: &ReviewState) -> Result<(), Self::Error> {
        Ok(())
    }

    fn save_capture(&mut self, capture: &Capture) -> Result<CaptureResult, Self::Error> {
        self.captures.push(capture.clone());
        Ok(CaptureResult {
            word_created: true,
            contexts_created: capture.contexts.len() as u64,
        })
    }

    fn save_captures(&mut self, captures: &[Capture]) -> Result<Vec<CaptureResult>, Self::Error> {
        captures
            .iter()
            .map(|capture| self.save_capture(capture))
            .collect()
    }

    fn compare_and_swap_review_state(
        &mut self,
        _expected: &ReviewState,
        _replacement: &ReviewState,
    ) -> Result<ReviewUpdate, Self::Error> {
        Ok(ReviewUpdate::Updated)
    }

    fn review_state(&self, word_id: &WordId) -> Result<Option<ReviewState>, Self::Error> {
        Ok(self
            .captures
            .iter()
            .find(|capture| capture.word.id == *word_id)
            .map(|capture| capture.initial_review_state.clone()))
    }

    fn due_reviews(&self, _as_of: DateTime<Utc>) -> Result<Vec<ReviewCard>, Self::Error> {
        Ok(Vec::new())
    }

    fn due_count(&self, _as_of: DateTime<Utc>) -> Result<u64, Self::Error> {
        Ok(self.captures.len() as u64)
    }

    fn review_statistics(&self, _as_of: DateTime<Utc>) -> Result<ReviewStatistics, Self::Error> {
        Ok(ReviewStatistics::default())
    }
}

struct FakeLexicon {
    entries: HashMap<String, LexiconEntry>,
}

impl FakeLexicon {
    fn immersion() -> Self {
        Self {
            entries: HashMap::from([(
                "immersion".to_string(),
                LexiconEntry {
                    lemma: "immersion".to_string(),
                    phonetic: "phonetic".to_string(),
                    definitions: vec![Definition {
                        part_of_speech: "noun".to_string(),
                        text: "deep involvement".to_string(),
                    }],
                },
            )]),
        }
    }
}

impl Lexicon for FakeLexicon {
    type Error = Infallible;

    fn lookup(&self, surface: &str) -> Result<Option<LexiconEntry>, Self::Error> {
        Ok(self.entries.get(&surface.to_lowercase()).cloned())
    }
}

#[test]
fn capture_effect_uses_shared_factory_and_refreshes_saved_state() {
    let mut storage = FakeStorage::default();
    let lexicon = FakeLexicon::immersion();

    let message = execute_effect(
        Effect::Capture {
            surface: "Immersion".to_string(),
            context: "Immersion works.".to_string(),
            source: "tui:/docs/a.md".to_string(),
            captured_at: now(),
        },
        &mut storage,
        &lexicon,
        Path::new("/docs"),
    );

    let Message::CaptureFinished(Ok(feedback)) = message else {
        panic!("expected successful capture message");
    };
    assert!(feedback.result.word_created);
    assert_eq!(
        feedback.details.entry.expect("entry must exist").lemma,
        "immersion"
    );
    assert!(feedback.details.state.is_some());
    assert_eq!(storage.captures[0].contexts[0].source, "tui:/docs/a.md");
}

#[derive(Clone)]
struct FakeTerminal {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail_on: Option<&'static str>,
}

impl FakeTerminal {
    fn record(&self, event: &'static str) -> io::Result<()> {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
        if self.fail_on == Some(event) {
            Err(io::Error::other(format!("failed at {event}")))
        } else {
            Ok(())
        }
    }
}

impl TerminalControl for FakeTerminal {
    type Error = io::Error;

    fn enable_raw(&mut self) -> Result<(), Self::Error> {
        self.record("enable_raw")
    }

    fn enter_alternate(&mut self) -> Result<(), Self::Error> {
        self.record("enter_alternate")
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.record("hide_cursor")
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.record("show_cursor")
    }

    fn leave_alternate(&mut self) -> Result<(), Self::Error> {
        self.record("leave_alternate")
    }

    fn disable_raw(&mut self) -> Result<(), Self::Error> {
        self.record("disable_raw")
    }
}

#[test]
fn terminal_guard_restores_every_successfully_enabled_state() {
    let events = Arc::new(Mutex::new(Vec::new()));
    {
        let terminal = FakeTerminal {
            events: events.clone(),
            fail_on: None,
        };
        let _guard = TerminalGuard::enter(terminal).expect("terminal must enter");
    }

    assert_eq!(
        events.lock().expect("events lock must succeed").as_slice(),
        [
            "enable_raw",
            "enter_alternate",
            "hide_cursor",
            "show_cursor",
            "leave_alternate",
            "disable_raw"
        ]
    );
}

#[test]
fn terminal_guard_cleans_up_after_partial_initialization_failure() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let terminal = FakeTerminal {
        events: events.clone(),
        fail_on: Some("hide_cursor"),
    };

    let result = TerminalGuard::enter(terminal);

    assert!(result.is_err());
    assert_eq!(
        events.lock().expect("events lock must succeed").as_slice(),
        [
            "enable_raw",
            "enter_alternate",
            "hide_cursor",
            "leave_alternate",
            "disable_raw"
        ]
    );
}
