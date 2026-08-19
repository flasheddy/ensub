# Product Requirements Document: Ensub Interactive Podcast & Audio Immersion

| | |
|---|---|
| **Product** | Ensub (`esb`) |
| **Milestone** | Milestone 5 |
| **Target release** | v0.2.0 |
| **Status** | Milestone 5 complete and verified |
| **Stable baseline** | Ensub Core v0.1.0 delivered and stable |
| **Verified repository baseline** | v0.1.0-rc1 merged and verified on `main` |
| **Primary surface** | Installable PWA/web player powered by `ensub-wasm` |
| **Date** | 2026-08-17 |
| **Supersedes** | [Ensub v0.1.0 PRD and legacy roadmap](archive/ensub-prd-v0.1.0.md) |

---

## 1. Executive Summary

Milestone 5 pivots Ensub's primary product experience from a collection of
general-purpose capture surfaces to an **Interactive Podcast & Audio Immersion
Player**. The v0.2.0 experience is an installable web application that lets a
learner open a transcript-enabled podcast, listen while the transcript follows
the audio, inspect an unfamiliar word with one tap, and turn that moment into a
review card grounded in the original sentence and audio.

The pivot builds on the delivered Ensub Core v0.1.0 foundation rather than
replacing it. The deterministic SRS engine, local lexicon, storage contracts,
SQLite adapter, CLI, and offline WASM sandbox remain stable. Milestone 5 makes
`ensub-wasm` the engine behind the primary user interface and adds portable
podcast, transcript, cue, and media-context capabilities for future clients.

The default learning loop remains local-first and usable without an LLM. A
remote contextual-disambiguation endpoint is optional, explicit, and invoked
only when the learner asks for it.

### 1.1 Product Promise

> Listen naturally, understand a word in place, save the exact moment, and
> review it with its original audio.

### 1.2 Product Principles

- **Audio and language stay together.** Every captured word retains the
  sentence, episode identity, transcript location, and playable time range
  that gave it meaning.
- **One tap before one form.** Lookup must be immediate; capture should require
  only an explicit confirmation, not manual metadata entry.
- **Offline is the default learning path.** Lemma lookup, saved captures, and
  SRS review do not depend on an LLM or cloud database.
- **Remote intelligence is optional.** Contextual disambiguation augments the
  local result on demand and never gates capture or review.
- **Portable rules live in Rust.** Parsing, cue models, cue selection, context
  construction, and audio-slice math must be reusable by a future UniFFI/mobile
  adapter without a JavaScript rewrite.
- **The web layer is a thin platform host.** It owns browser transport, audio
  DOM events, media controls, scrolling, and rendering, but not transcript or
  learning-domain rules.

---

## 2. Milestone 4 Closure and Stable Baseline

Milestone 4 is closed. **Ensub Core v0.1.0 is delivered and stable**, with
`v0.1.0-rc1` as the merged and verified repository baseline.

| Baseline capability | Status at M5 start | M5 posture |
|---|---|---|
| `esb` CLI capture, parsing, due counts, statistics, and review | Delivered and stable | Maintain compatibility |
| Native SQLite engine, migrations, bundled lexicon, and shared storage contracts | Delivered and stable | Reuse; evolve storage through versioned migrations |
| Portable domain, morphology, lexicon, and deterministic SM-2 policy | Delivered and stable | Extend without changing dependency direction |
| Offline Ensub Core WASM sandbox with local capture, review, and persistence | Delivered and stable | Reuse as the browser engine foundation |
| TUI, COSMIC GUI, and COSMIC applet | Delivered v0.1 surfaces | Maintenance only during M5 |

No unfinished Milestone 4 product requirement gates Milestone 5. Release
publication or packaging operations are separate from this product scope.

---

## 3. Problem and Opportunity

Podcast listening exposes learners to natural pronunciation, pacing, and
context, but the learning workflow is fragmented. A learner commonly has to
pause the player, search a dictionary, reconstruct the sentence, remember the
episode location, and later review a silent text card. That interruption makes
capture expensive and disconnects review from the sound that made the word
memorable.

Podcasting 2.0 feeds can publish time-aligned transcripts through
`<podcast:transcript>` tags. Ensub can combine those transcripts with its local
lexicon and SRS engine to make the listening moment itself the durable learning
artifact.

### 3.1 Primary Users

- Intermediate and advanced English learners who listen to transcript-enabled
  podcasts and want low-friction vocabulary capture.
- Privacy-conscious learners who want local lookup and SRS without creating a
  cloud account.
- Existing Ensub users who want the same lemma, context, and review behavior in
  an audio-first interface.

### 3.2 Core Jobs To Be Done

1. When I hear or read an unfamiliar word, help me understand it without
   leaving the player.
2. When I save a word, retain the sentence and exact audio moment so I do not
   have to reconstruct the context later.
3. When the local dictionary has several plausible senses, let me request a
   contextual explanation without making AI mandatory.
4. When a word becomes due, let me hear the original moment immediately while
   I review it.

---

## 4. Milestone Goals

Milestone 5 succeeds when Ensub can:

1. Ingest an RSS feed and discover episode transcripts advertised with
   `<podcast:transcript>`.
2. Parse WebVTT and SRT into one normalized, validated cue model.
3. Play episode audio while highlighting and auto-scrolling the synchronized
   transcript.
4. Resolve a tapped transcript word through the existing offline
   `ensub-wasm` lexicon pipeline.
5. Capture the word with sentence context, structured episode provenance, cue
   provenance, and a deterministic padded audio range.
6. Request contextual disambiguation from an optional LLM endpoint without
   weakening the offline default.
7. Run SRS reviews inside the player and play each card's saved audio range
   without navigating away.
8. Keep all reusable parsing and time-range behavior in portable Rust so a
   future mobile client binds the same implementation.

---

## 5. Primary User Journeys

### 5.1 Add a Podcast and Choose an Episode

1. The learner enters an RSS feed URL.
2. The browser host fetches the feed and passes the response bytes and final
   source URL into `ensub-wasm`.
3. Rust parses the feed, returns normalized feed and episode records, and marks
   supported transcript resources.
4. The learner chooses an episode with a WebVTT or SRT transcript.
5. The host fetches the transcript and assigns the enclosure URL to the browser
   audio element. Rust parses and validates the transcript before synchronized
   playback opens.
6. The app remembers the feed and cached episode metadata locally.

### 5.2 Listen, Look Up, and Capture

1. Audio playback advances and the active transcript cue is highlighted.
2. The transcript follows playback unless the learner has temporarily taken
   manual scroll control.
3. The learner taps a word in the transcript.
4. `ensub-wasm` returns the local lemma, pronunciation, part of speech, and
   ranked definitions without a network request.
5. The learner confirms capture with one action.
6. Ensub stores the lexical record, sentence context, episode and transcript
   metadata, cue range, and padded audio-slice descriptor atomically.

### 5.3 Ask for Contextual Disambiguation

1. The learner opens a local lookup result and explicitly selects the
   contextual explanation action.
2. Ensub shows what context will be sent and invokes the configured optional
   endpoint.
3. The endpoint returns the most likely sense and a concise explanation.
4. Failure, timeout, or missing configuration leaves the local lookup and
   capture flow fully usable.

### 5.4 Review in the Player

1. The learner opens the due queue without leaving the player.
2. A review card presents the saved word and episode context according to the
   existing review phase.
3. The learner can play or replay the exact saved audio range immediately.
4. The learner reveals the answer and submits the existing 0-5 recall rating.
5. Rust updates the SM-2 state and advances to the next due card.
6. On exit, the player returns to the prior episode and playback position.

---

## 6. Functional Requirements

### 6.1 RSS and Transcript Ingestion

**M5-FR-001 - Feed input.** The PWA must accept an RSS feed URL and present a
normalized episode list. The initial v0.2.0 contract covers RSS 2.0 feeds; a
podcast directory or search catalog is not required. Direct URL ingestion in
v0.2.0 supports feeds and transcripts that permit browser access through CORS.
When the Fetch API exposes only an opaque network failure, Ensub must report
that the browser could not access the source and that CORS or another network
policy may be the cause; it must not claim a cause the browser cannot prove or
route the request through an implicit cloud proxy.

**First-run / empty state.** When no feeds exist, the player workspace must
provide a 'Load Demo Episode' action using bundled, pre-parsed synthetic fixture
data, allowing immediate offline verification without hunting for external RSS
URLs. The fixture at `assets/demo-fixture.json` must describe a synthetic
episode titled `"Ensub Demo: Natural English Context"` with an approximately
two-minute `demo.mp3` source and 15-20 WebVTT cues containing approximately 250
words total. It must include pre-calculated cue timings and Rust-compatible
UTF-16 token spans so synchronized playback and offline lookup can be verified
immediately without parsing or network access.

**M5-FR-002 - Portable feed parsing.** The browser host may perform the HTTP
fetch because transport is platform I/O, but feed XML interpretation must run
in portable Rust. JavaScript must pass raw bytes plus source URL and receive
structured DTOs.

**M5-FR-003 - Podcast transcript discovery.** The parser must recognize
namespace-qualified `<podcast:transcript>` elements attached to episodes and
retain at least these attributes when present:

- transcript URL;
- MIME type;
- language;
- relation/type metadata supplied by the feed.

**M5-FR-004 - Supported transcript formats.** v0.2.0 must support:

- WebVTT advertised as `text/vtt`;
- SRT advertised as `application/x-subrip`, `application/srt`, or `text/srt`.

MIME matching must be case-insensitive and may ignore parameters such as a
declared character set. Unsupported transcript types remain visible but cannot
be selected as playable transcripts.

**M5-FR-005 - Episode provenance.** Normalized episode data must retain:

- feed URL and feed title;
- stable episode GUID when supplied;
- episode title;
- publication time when supplied;
- audio enclosure URL and MIME type;
- duration and artwork when supplied;
- every advertised transcript resource.

Missing optional metadata must not make an otherwise playable episode fail.
Every episode must also receive a stable internal identity. Rust must derive it
from the canonical feed URL and publisher GUID when a GUID is present, or from
the canonical feed URL and canonical audio enclosure URL otherwise, then
persist it with feed-scoped GUID and enclosure aliases. On refresh, a match by
either alias retains the existing internal identity and merges newly observed
aliases. If a publisher later adds a GUID, Ensub must reconcile it through the
existing enclosure alias rather than create a second episode. The internal
identifier must not be presented as publisher-supplied metadata.

**M5-FR-006 - Transcript selection.** If an episode exposes multiple supported
transcripts, Ensub must prefer the learner's selected language, then the feed or
episode language, and otherwise ask the learner to choose. The chosen resource
must be persisted with the episode state.

**M5-FR-007 - Ingestion failures.** Invalid URLs, known HTTP failures,
redirects to unsupported schemes, opaque browser-access failures, oversized
responses, malformed XML, and unsupported transcript formats must produce
distinct actionable states when browser APIs expose the distinction. DNS, TLS,
CORS, and network-policy failures that Fetch reports identically must share a
truthful browser-access failure state. The UI must never imply that a parsing
change can bypass a browser network policy.

**M5-FR-008 - Local cache.** Successfully parsed feed metadata, episode
metadata, transcript resource metadata, and normalized cues must be stored
locally so an already opened transcript can be revisited without refetching it.

### 6.2 Transcript Parsing and Cue Synchronization

**M5-FR-009 - Normalized cue model.** WebVTT and SRT parsers must produce the
same ordered cue representation with:

- stable cue identity within the transcript;
- zero-based source order;
- integer `start_ms` and `end_ms` values;
- normalized display text;
- Rust-produced word tokens with cue-relative text spans;
- source format and optional source cue identifier.

**M5-FR-010 - Validation.** Rust must reject the transcript document with a
typed cue/line error when a cue timestamp is unparseable, timestamp arithmetic
overflows, or `end_ms <= start_ms`. Empty-text cues are discarded after their
timings have been validated. The parser must tolerate common line-ending
variants, UTF-8 byte order marks, WebVTT metadata blocks, SRT numeric indices,
and cue text spanning multiple lines.

**M5-FR-011 - Safe text.** Cue payloads are untrusted content. Parsing must
remove or normalize supported caption markup into text spans. The PWA must
render transcript content as text, never as unsanitized HTML.

**M5-FR-012 - Active cue resolution.** The browser host must pass the current
media position into a Rust cue-selection API. Rust returns every active cue in
source order. The web layer must not duplicate timestamp-boundary rules.

The active interval is `start_ms <= position_ms < end_ms`. At a shared boundary,
the ending cue is inactive and the starting cue is active. Overlapping cues are
all active; the first active cue in source order is the auto-scroll anchor. A
gap returns an empty active set and leaves the nearest prior cue visible but not
highlighted. Seeking uses the same rules as continuous playback.

**M5-FR-013 - Synchronized transcript.** During playback, the PWA must:

- visibly distinguish the active cue;
- keep it in a stable reading region through auto-scroll;
- update promptly after play, seek, rate change, and time-update events;
- let the learner select a cue to seek audio to that cue's start;
- preserve text selection and lookup interaction while synchronization runs.

**M5-FR-014 - Manual-scroll pause.** User-initiated transcript scrolling must
temporarily suspend auto-scroll so the interface does not fight the learner.
A clear return-to-current-cue control must resume following immediately, and
normal playback may resume follow mode after a documented idle interval.

**M5-FR-015 - Media controls.** The primary player must provide play/pause,
seek, elapsed and remaining time, playback speed, volume/mute where supported,
short backward/forward skips, episode identity, and transcript follow state.
Controls must remain usable by keyboard and assistive technology.

### 6.3 One-Tap Offline Lemma Lookup

**M5-FR-016 - Tap target.** Each Rust-produced transcript word token must be a
usable pointer and keyboard target without altering the underlying cue text.
Portable Rust may retain native string offsets internally, but `ensub-wasm`
must expose a half-open cue-relative UTF-16 code-unit range for each token so
the PWA can map it to a JavaScript/DOM string without retokenizing.

**M5-FR-017 - Existing engine reuse.** A tap must call the existing
`ensub-wasm` language and lexicon pipeline. The PWA must not maintain a second
JavaScript tokenizer, morphology table, or dictionary.

**M5-FR-018 - Lookup result.** A successful local lookup must expose the
surface form, normalized lemma, pronunciation when available, part of speech,
and ranked local definitions. An unknown word must have a deliberate state that
reports no local entry rather than a fabricated result.

**M5-FR-019 - Offline behavior.** After the PWA shell and lexicon assets have
been installed, local lookup must work without network access. Lookup must not
silently invoke the optional LLM endpoint.

**M5-FR-020 - Capture affordance.** The lookup surface must offer one explicit
capture action. Opening a lookup does not itself create or modify a card.

### 6.4 Contextual Capture and Audio Slices

**M5-FR-021 - Sentence context.** A capture must store the transcript sentence
containing the selected token. When a sentence crosses cue boundaries, Rust
must expand across adjacent cues until it finds the complete sentence. If the
source transcript has no recoverable sentence boundary, Ensub must preserve the
nearest non-empty cue window without inventing text and mark it as a fallback
context. Cross-cue sentence expansion must not search beyond 3 adjacent cues
(or a maximum of 60 words) in either direction. If no terminal punctuation is
encountered within this window, the fallback cue window is saved.

**M5-FR-022 - Structured source metadata.** Each podcast capture must retain,
as structured fields rather than one overloaded source string:

- feed URL and title;
- stable internal episode identity, episode GUID when present, and title;
- episode publication time when present;
- audio enclosure URL;
- transcript URL and format, plus language when present;
- selected token and normalized lemma;
- first and last source cue identity;
- capture playback position;
- capture time supplied by the host.

**M5-FR-023 - Audio-slice range.** Every podcast capture must store a validated
logical audio range derived in Rust. Let `start` be the start of the first cue
used for the saved context and `end` be the end of the last cue used for it:

```text
slice_start_ms = max(0, start_ms - 500)
slice_end_ms   = min(media_duration_ms, end_ms + 500)  // when duration is known
```

When duration is unknown, `slice_end_ms` is `end_ms + 500` using checked
integer arithmetic. The resulting range must satisfy
`0 <= slice_start_ms < slice_end_ms`.

For a single-cue sentence, `start_ms` and `end_ms` are that cue's bounds. For a
sentence spanning multiple cues, they are the earliest start and latest end in
the saved cue range.

**M5-FR-024 - Logical slice, not transcoding.** v0.2.0 stores an audio source
reference plus start/end timestamps. It does not copy, transcode, or export a
new audio file. Playback seeks the source media to `slice_start_ms` and stops at
`slice_end_ms`; it may use a locally cached source when available. Snippet
stopping at `slice_end_ms` uses standard DOM audio event listeners
(`timeupdate` / `requestAnimationFrame`) with a practical tolerance threshold
(±50 ms). Frame-accurate sample-level audio buffering is not required for
v0.2.0.

**M5-FR-025 - Atomic persistence.** The word, lexical data, sentence, episode
source, cue provenance, audio slice, and initial SRS state must be persisted as
one logical capture. A partial media-context record must not be left behind if
the capture fails.

**M5-FR-026 - Repeat encounters.** Capturing an existing lemma from another
episode must retain the new media context without creating an unintended
duplicate learning identity. The learner must be able to choose among saved
contexts during later review.

### 6.5 Optional Contextual Disambiguation

**M5-FR-027 - Explicit invocation.** Contextual disambiguation runs only after
the learner selects an explicit action for the current word and sentence. It
must never run on episode import, transcript playback, lookup, or capture by
default.

**M5-FR-028 - Provider-neutral endpoint.** The integration must target a
configured, OpenAI-compatible or otherwise documented provider-neutral
endpoint through a replaceable adapter. No provider credential may be embedded
in committed PWA assets, WASM, source code, logs, or local sample data.

**M5-FR-029 - Minimal request.** The request must contain only the selected
word, saved sentence, candidate local senses needed for disambiguation, and the
minimum episode label useful to the learner. Sending a full transcript or
audio is outside the default contract.

**M5-FR-030 - Result.** A successful response should identify the likely local
sense when possible and provide a concise context-specific explanation. The
result must be visually distinguished from bundled lexicon content.

**M5-FR-031 - Failure isolation.** Missing endpoint configuration, offline
state, authentication errors, rate limits, timeouts, invalid responses, and
provider failures must leave local lookup, capture, playback, and review
available. The learner can retry explicitly.

**M5-FR-032 - Consent and retention.** Before the first request to an endpoint,
the PWA must explain what text leaves the device. LLM responses are not required
for card validity and must not replace the stored local definition silently.

### 6.6 In-Player SRS Review

**M5-FR-033 - Shared scheduler.** The player must query and update due cards
through the existing `core_engine` storage and SM-2 contracts. It must not
implement a web-specific scheduling algorithm.

**M5-FR-034 - Player-contained session.** A learner must be able to start,
complete, and leave a review session without navigating to a separate product
surface or losing the current episode and playback position. The PWA must keep
one `<audio>` element across player and review states:

- On entering review, it pauses the active episode and pushes
  `{ episode_id, currentTime_ms, playbackRate }` onto a saved session stack.
- During review, the overlay reuses that element to seek from
  `slice_start_ms` to `slice_end_ms` for snippet playback.
- On exiting review, it pops the saved state, restores the previous episode
  source and playback rate, seeks to the saved timestamp, and leaves the audio
  element paused until the learner explicitly resumes playback.

**M5-FR-035 - Immediate snippet playback.** A podcast-backed review card must
offer a prominent play/replay control that seeks to the saved range and stops
at its end. Snippet stopping at `slice_end_ms` uses standard DOM audio event
listeners (`timeupdate` / `requestAnimationFrame`) with a practical tolerance
threshold (±50 ms). Frame-accurate sample-level audio buffering is not required
for v0.2.0. Repeated playback must not change SRS state.

**M5-FR-036 - Review context.** The card must be able to show the saved sentence,
episode title, lemma, pronunciation, definition, and timestamp. The normal
prompt/reveal sequence must avoid exposing the configured answer before reveal.

**M5-FR-037 - Rating and transition.** The existing validated 0-5 recall rating
updates the card atomically with a host-supplied timestamp. The next due card
must be ready without reloading the episode or application.

**M5-FR-038 - Graceful degradation.** If the original audio URL is unavailable
and no local media is cached, the review remains completable with its saved text
context and shows that audio is unavailable. A media failure must not corrupt or
block the SRS transition.

---

## 7. Product Experience Requirements

### 7.1 Primary Player Workspace

The first application screen is the usable player, not a marketing landing
page. It contains these coordinated regions:

- episode identity and compact feed navigation;
- stable audio controls;
- the time-aligned transcript as the dominant reading surface;
- a lookup/capture surface that does not obscure the active sentence;
- access to the due queue and an in-player review state.

When no feeds exist, this workspace must surface the 'Load Demo Episode' action
as its primary empty-state action and open the bundled, pre-parsed synthetic
fixture without requiring network access.

The exact responsive composition may vary, but playback controls and the active
cue must remain visible or immediately reachable on both mobile-sized and
desktop browser viewports.

### 7.2 Required States

The PWA must deliberately render:

- feed loading, empty, invalid, unavailable, and browser-access-failed states;
- episode with no transcript;
- episode with only unsupported transcript formats;
- transcript loading, malformed, cached, and offline states;
- audio loading, stalled, ended, and unavailable states;
- local lookup success, ambiguity, and no-entry states;
- capture pending, saved, duplicate encounter, and failed states;
- LLM unconfigured, requesting, complete, and failed states;
- review empty, prompt, revealed, rated, and audio-unavailable states.

### 7.3 Accessibility and Interaction

Keyboard commands must follow this mapping within the named interaction scope
and must not intercept keystrokes entered in editable controls:

| Scope | Key | Action |
|---|---|---|
| Global player | `Space` | Play or pause |
| Global player | `J` / `Down` | Move to the next cue |
| Global player | `K` / `Up` | Move to the previous cue |
| Global player | `Left` / `Right` | Skip backward 5 seconds or forward 5 seconds |
| Global player | `[` / `]` | Decrease or increase playback speed |
| Global player | `R` | Open or close the review queue |
| Transcript / lookup | `Tab` / `Shift+Tab` | Move focus between word tokens |
| Transcript / lookup | `Enter` | Open lookup for the focused token |
| Transcript / lookup | `C` | Confirm capture |
| Transcript / lookup | `Escape` | Dismiss lookup and clear token focus |
| Review session | `Space` / `P` | Replay the saved audio snippet |
| Review session | `Enter` | Reveal the answer |
| Review session | `0`-`5` | Submit the corresponding SM-2 rating |
| Review session | `Escape` | Close review and return to the player |

- Media controls must have programmatic names and visible focus states.
- Active-cue styling cannot rely on color alone.
- Auto-scroll must respect manual navigation and reduced-motion preferences.
- Transcript text must remain selectable and readable by assistive technology.
- Focus must move predictably when lookup and review overlays open or close.

---

## 8. Portable Data Model

The exact Rust names may evolve during design, but Milestone 5 requires typed
equivalents of the following records:

| Record | Required responsibility |
|---|---|
| `PodcastFeed` | Canonical source URL, title, language, and feed-level metadata |
| `EpisodeIdentity` | Persisted internal ID plus feed-scoped publisher GUID and enclosure aliases used for refresh reconciliation |
| `PodcastEpisode` | Stable internal identity, feed relationship, optional publisher GUID, title, publication time, enclosure, duration, and artwork |
| `TranscriptResource` | URL, normalized supported format, optional language, and relation metadata |
| `TranscriptCue` | Stable identity, source order, integer start/end milliseconds, and normalized text |
| `TranscriptToken` | Surface text plus a cue-relative half-open span produced by Rust tokenization |
| `TranscriptDocument` | Transcript provenance plus validated ordered cues |
| `CueRange` | First/last cue identity and the context time bounds |
| `AudioSlice` | Audio source reference and validated padded start/end milliseconds |
| `PodcastContext` | Sentence, context quality, episode/transcript provenance, cue range, playback position, and audio slice |

Podcast provenance must extend the existing context model; it must not be
encoded solely inside the current generic `source` string. Existing v0.1
captures must remain readable after a versioned storage migration.

All media arithmetic uses non-negative integer milliseconds. Portable domain
logic accepts playback position, media duration, capture time, and review time
as arguments. It must not read the system clock or browser media state.

---

## 9. Architecture and Ownership

### 9.1 Rust Ownership

There is no Cargo package named `ensub-core`. In this document, **Ensub Core**
names the delivered product foundation. The portable domain package is
`core_engine` at `crates/core_engine`, and the browser package is `ensub-wasm`
at `crates/wasm_bridge`.

| Layer | Milestone 5 ownership |
|---|---|
| `crates/core_engine` (`core_engine`) | Feed/episode references needed by captures, cue and audio-slice domain records, validation, active-cue selection, padded slice math, podcast-context storage contracts, and existing SRS policy |
| `crates/language_engine` (`language_engine`) | Portable RSS, WebVTT, and SRT parsing; transcript text normalization; tokenization; sentence reconstruction across cues; existing morphology and lexicon contracts |
| `crates/wasm_bridge` (`ensub-wasm`) | Browser-facing DTOs and bindings for the portable APIs, UTF-16 token-span conversion, error conversion, snapshot-schema evolution, and browser storage adapter integration |
| PWA/web host | Browser fetch/cache transport, `<audio>` element ownership, DOM media events, controls, transcript rendering, auto-scroll behavior, focus, and responsive UI |
| Optional LLM adapter | Explicit endpoint transport, request/response validation, consent state, and provider error mapping outside portable core policy |

WebVTT/SRT parsing, cue data models, active-cue boundary behavior, sentence
context construction, and audio-slice timestamp math must be implemented once
in Rust. `ensub-wasm` exposes them; it must not become the only home of logic
required by a future UniFFI client.

### 9.2 Thin Web Host Rule

The PWA may use browser APIs for work that cannot be portable:

- fetch RSS and transcript resources and assign audio sources to the media
  element;
- observe media position, duration, play state, and errors;
- cache resources through browser storage and service-worker APIs;
- render controls, transcript cues, lookup results, and review states;
- apply scrolling and focus effects requested by UI state.

The PWA must pass raw feed/transcript content and explicit media timestamps into
Rust. It must not parse RSS/WebVTT/SRT, derive sentence boundaries, calculate
audio padding, lemmatize tokens, schedule reviews, or invent a parallel
persistence schema in JavaScript.

### 9.3 Data Flow

```text
feed URL
  -> browser fetch adapter
  -> raw RSS bytes + source URL
  -> portable Rust feed parser
  -> episode + transcript resource DTOs

transcript URL
  -> browser fetch adapter
  -> raw WebVTT/SRT text
  -> portable Rust transcript parser
  -> validated cue model
  -> PWA rendering

audio DOM event(position, duration)
  -> Rust active-cue selection
  -> PWA highlight/scroll effect

token selection + cue range
  -> Rust lemma/context/slice construction
  -> versioned local capture
  -> shared SRS queue
```

### 9.4 Dependency Boundaries

- `core_engine` remains free of UI, browser, database-driver, WASM, network,
  XML-parser, and async-runtime dependencies.
- `language_engine` remains portable and free of DOM, browser storage, native
  SQLite, and UI dependencies.
- `ensub-wasm` depends inward on portable crates; native SQLite, COSMIC, TUI,
  and native-only LLM dependencies must not enter the WASM graph.
- Parsers and domain functions return typed, actionable errors and do not
  panic on untrusted feed or transcript input.
- Future mobile/UniFFI work binds `core_engine` and `language_engine`; it does
  not port JavaScript business rules.

---

## 10. Offline, Network, Privacy, and Persistence

### 10.1 Capability Matrix

| Workflow | Offline requirement |
|---|---|
| Open installed PWA shell | Required after first successful install |
| Look up a lemma with the bundled lexicon | Required |
| Open a previously cached transcript | Required |
| Capture into local browser storage | Required |
| Run SRS review and update scheduling | Required |
| Replay a slice whose source audio is locally cached | Required |
| Import or refresh a remote feed/transcript | Network required |
| Stream uncached episode audio | Network required |
| Replay a slice whose source audio is not cached | Network required |
| Request optional LLM disambiguation | Network and configured endpoint required |

Whole-episode offline download management is not a v0.2.0 release requirement.
The logical audio-slice model must remain compatible with later download
support.

### 10.2 Local Persistence

- Feed subscriptions, episode state, parsed transcripts, captures, media
  contexts, and SRS state remain on the device by default.
- The browser snapshot/schema change must be versioned and migration-tested.
- Migration must preserve valid v0.1 captures and review state.
- Storage migrations must be atomic: the migrated snapshot must be fully
  constructed and validated before it replaces the active schema, and no
  partial migration may become visible.
- Before a v0.1 migration begins, the uncorrupted source snapshot must be
  preserved under `ensub_v0.1_backup`.
- If a schema migration fails, the app must abort the migration immediately,
  preserve `ensub_v0.1_backup`, enter read-only mode, and reject all write
  operations. It must present an explicit recovery banner with an action to
  export the raw JSON snapshot; it must not silently reset or wipe the store.
- The product must expose a local reset/export path consistent with existing
  Ensub Core privacy behavior before v0.2.0 release.

**Lexicon Asset Strategy.** The compressed lexicon must ship as a versioned
static sidecar asset (`assets/lexicon-v1.*`) pre-cached by the Service Worker on
initial load. It must not be embedded directly into the compiled `.wasm` binary
to avoid bloating WASM initialization time.

### 10.3 Privacy

- No account, telemetry service, or cloud database is required for M5.
- Feed, transcript, capture, and review data must not be sent to the optional
  LLM endpoint automatically.
- Provider secrets must never be embedded in browser-delivered assets.
- Logs and errors must not include complete transcripts, credentials, or
  private endpoint tokens.
- Test fixtures, screenshots, and examples use public-domain or synthetic feed,
  transcript, episode, and learner data.

---

## 11. Quality Requirements

### 11.1 Reliability and Security

- Treat feed XML, caption text, URLs, episode metadata, and endpoint responses
  as untrusted input.
- Bound accepted feed/transcript sizes and cue counts, returning a typed limit
  error instead of exhausting browser memory.
- Use checked timestamp arithmetic and validate all ranges at construction.
- Render external text without executable markup.
- Preserve usable local state across malformed input, media errors, endpoint
  failures, and application reloads.
- Do not allow a failed network request to corrupt a previously cached feed or
  transcript.

### 11.2 Performance

- Media time updates must not trigger a full linear scan of the transcript on
  every event; active-cue lookup must use an indexed or equivalent bounded
  strategy.
- Active-cue highlighting must keep pace with normal playback and update on the
  first rendered state after a seek.
- Token lookup and opening a locally stored review card must feel immediate and
  must not wait for a network request.
- Transcript virtualization may be used in the PWA for long episodes, but it
  must preserve selection, accessibility, and active-cue positioning.

### 11.3 Testability

- RSS, WebVTT, and SRT parsers require fixture-driven unit tests for valid,
  malformed, boundary, and unsupported inputs.
- Cue selection and audio-slice math require pure table-driven tests, including
  zero clamping, known-duration clamping, multi-cue contexts, gaps, overlaps,
  and checked-overflow cases.
- WASM bindings require round-trip DTO and browser-target tests.
- Token-span tests must cover punctuation and non-ASCII text and prove that the
  UTF-16 ranges exposed to JavaScript select the original surface forms.
- Storage requires forward migration and failed-migration recovery tests from
  the v0.1 browser snapshot.
- Browser tests require real audio-event simulation or deterministic media
  fixtures for play, pause, seek, rate change, cue following, manual scrolling,
  lookup, capture, reload, offline mode, and review snippet boundaries.
- Tests must prove that local lookup, capture, and review remain usable when the
  optional endpoint is absent or failing.

---

## 12. Frozen and Out of Scope for v0.2.0

The following areas are explicitly frozen for Milestone 5. Maintenance,
compatibility, and security fixes are allowed, but no v0.2.0 feature capacity
is allocated to them:

- **Native mobile rewrites.** No iOS or Android application, mobile UI rewrite,
  or UniFFI shipping target. M5 only preserves portable Rust boundaries for a
  future target.
- **Cloud sync databases.** No multi-device synchronization, hosted learning
  database, account system, conflict service, or cloud backup.
- **Desktop GUI/applet additions.** No new COSMIC GUI or applet podcast player,
  transcript workflow, capture flow, or review feature.
- **Browser extensions.** No extension packaging, content script, store
  submission, or web-page capture feature.

Also not required for v0.2.0:

- automatic speech recognition or transcript generation for episodes that do
  not publish WebVTT/SRT;
- audio transcoding, destructive editing, clip-file export, or waveform
  editing;
- a public podcast search directory, ratings, social features, or creator
  analytics;
- automatic/background LLM analysis;
- authenticated or DRM-protected feed support;
- whole-episode offline download management.

Existing CLI, TUI, GUI, applet, offline sandbox, and optional Ensub Context
surfaces are not removed by this pivot. They remain the v0.1 baseline while the
PWA player becomes the primary product interface.

---

## 13. Acceptance Criteria

Milestone 5 is accepted only when all of the following are true:

1. Product documentation identifies Ensub Core v0.1.0 as delivered and stable,
   with `v0.1.0-rc1` recorded as the verified repository baseline.
2. A browser end-to-end fixture server exposes a synthetic RSS 2.0 feed and
   transcript with explicit CORS permission. Entering that feed URL in the PWA
   imports normalized feed, episode, enclosure, and transcript-resource records;
   a second fixture without CORS produces a browser-access failure that names
   CORS or network policy only as a possible cause.
3. WebVTT and SRT fixtures representing the same captions produce equivalent
   normalized cue timings and text; malformed and unsupported inputs return
   typed errors without panics.
4. The browser player highlights the Rust-selected active cue set while
   playing, follows the first active cue during overlaps, shows no highlight in
   gaps, updates after a seek or rate change, auto-scrolls without layout jumps,
   and yields to manual transcript navigation.
5. Selecting a transcript word through its Rust-produced UTF-16 token span
   returns the existing offline lemma, pronunciation, part of speech, and local
   definitions after the application is placed offline.
6. One explicit action captures the word with its sentence, structured feed and
   episode metadata, transcript provenance, cue range, playback position, and
   logical audio slice. A feed fixture without an episode GUID remains
   capturable through the deterministic internal episode identity. When a
   refresh of that fixture adds a GUID, alias reconciliation preserves the same
   internal identity, cached episode state, and captures.
7. Audio-slice tests prove at least these rules:
   - cue `1000..2000 ms` becomes `500..2500 ms` when duration permits;
   - cue `200..1000 ms` becomes `0..1500 ms`;
   - padded end is clamped to a known media duration;
   - a multi-cue sentence uses its earliest cue start and latest cue end;
   - overflow and invalid ranges return typed errors.
8. Existing v0.1 browser captures and review state survive the versioned M5
   storage migration.
9. Contextual disambiguation is absent from the automatic path, requires
   explicit invocation and first-use disclosure, sends a minimal payload, and
   fails without blocking local lookup or capture.
10. A due podcast card can play/replay only its saved audio range, accept a
    validated 0-5 rating, persist the shared SM-2 transition, and advance
    without leaving the player.
11. A missing remote audio source degrades the review card to saved text without
    preventing a rating or corrupting review state.
12. PWA shell, cached transcript, lemma lookup, capture, and SRS review pass an
    offline browser test; uncached network dependencies are labeled honestly.
13. Dependency boundaries are verified by running
    `cargo tree -p ensub-wasm --target wasm32-unknown-unknown` and asserting zero
    occurrences of `rusqlite`, `libcosmic`, `reqwest`, `tokio`, `ensub-llm`, or
    native UI crates.
14. No native mobile, cloud-sync database, desktop GUI/applet addition, or
    browser-extension deliverable is included in the v0.2.0 release scope.

---

## 14. Delivery Sequence

The internal delivery order for Milestone 5 is:

1. **M5.1 - Portable media foundation:** Add typed episode, transcript, cue,
   media-context, and audio-slice records; implement validation, cue selection,
   and padded-range math in Rust.
2. **M5.2 - Portable ingestion:** Implement RSS transcript discovery and
   WebVTT/SRT parsing in `language_engine`, then expose DTOs and typed errors
   through `ensub-wasm`.
3. **M5.3 - Player workspace:** Build the installable PWA host, browser fetch
   adapters, audio controls, synchronized transcript, follow behavior, and
   explicit loading/error states.
4. **M5.4 - Lookup and media capture:** Integrate one-tap offline lookup,
   cross-cue sentence construction, structured podcast provenance, snapshot
   migration, and logical audio slices.
5. **M5.5 - In-player learning loop:** Add optional contextual disambiguation,
   due queue, prompt/reveal flow, instant snippet playback, and rating updates.
6. **M5.6 - Release hardening:** Complete offline, accessibility, performance,
   security, migration, cross-browser, and dependency-boundary validation.

Each slice must leave the portable crates independently testable and must not
move business rules into the PWA to accelerate a UI milestone.

---

## 15. Release Gates

Before v0.2.0 is considered complete:

- all acceptance criteria in this PRD pass;
- no placeholder, mock dictionary, mock persistence, or production panic path
  remains in the M5 workflow;
- Rust formatting, workspace checks, warning-free Clippy, workspace tests, and
  portable dependency-tree inspections pass;
- `wasm32-unknown-unknown` check and Clippy, `wasm-pack` browser tests, static
  PWA build verification, browser end-to-end tests, and WASM dependency-tree
  inspection pass;
- storage migration and rollback/recovery behavior are documented and tested;
- feed/transcript transport limitations and offline capability boundaries are
  documented for users;
- privacy documentation describes local podcast metadata, cached transcripts,
  logical audio slices, and optional endpoint disclosure;
- the repository secret scanner passes with no provider credentials or private
  feed data in source, fixtures, generated assets, screenshots, or logs.
