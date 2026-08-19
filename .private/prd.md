# Product Requirements Document: Milestone 6 - Native Android Client (v0.3.0)

| | |
|---|---|
| **Product** | Ensub (`esb`) |
| **Milestone** | Milestone 6 - Native Android Client |
| **Target release** | v0.3.0 (Native Android MVP; distribution unassigned) |
| **Status** | Active |
| **Primary surface** | Native Android application |
| **Tech stack** | Kotlin, Jetpack Compose, AndroidX Media3 (`MediaSessionService`), UniFFI mobile facade (`ensub-uniffi`), and Rust-owned SQLite |
| **Minimum Android version** | API 26 |
| **Supported ABIs** | `arm64-v8a` and `x86_64` |
| **Delivered baseline** | Android Spike 1, tagged `v0.3.0-spike1` |
| **Frozen reference** | [Milestone 5 / v0.2.0 PWA PRD](archive/ensub-prd-v0.2.0.md) |
| **Date** | 2026-08-19 |

---

## 1. Executive Summary

Milestone 6 makes native Android the active Ensub client track. The v0.3.0
MVP turns the delivered Kotlin, Compose, Media3, and UniFFI integration spike
into a lifecycle-correct mobile player with local podcast ingestion, durable
learning state, contextual vocabulary capture, and in-player review.

The Android client is a parallel platform adapter over the shared Rust core. It
does not replace the frozen v0.2.0 PWA or weaken its tests. Portable language,
media, scheduling, and persistence rules remain in Rust; Android owns the
platform behaviors that require Kotlin or Android framework APIs.

Distribution is intentionally unassigned for v0.3.0. The milestone establishes
a release-quality Android artifact and verification baseline without committing
the project to Google Play, an alternative store, or direct APK distribution.

### 1.1 Product Promise

> Listen naturally on Android, keep playback alive when the screen changes,
> understand and save a word in its original moment, and review it with the
> same audio context.

### 1.2 Product Principles

- **Native lifecycle first.** Playback belongs to a media service and remains
  coherent across Activity recreation, backgrounding, audio interruptions, and
  system controls.
- **One portable policy.** Parsing, cue selection, time math, lookup, capture,
  scheduling, and persistence behavior must not be reimplemented in Kotlin.
- **Local-first learning.** Ingestion may require network transport, but saved
  metadata, transcripts, lookup, capture, and review remain usable through
  local Rust-owned storage.
- **Thin mobile facade.** `ensub-uniffi` exposes stable, mobile-oriented DTOs
  and operations without leaking Rust implementation details into Compose.
- **Frozen means protected.** The PWA remains a tested behavioral reference,
  not an Android feature backlog or a target for new client features.

## 2. Goals and Success Definition

Milestone 6 succeeds when Ensub can:

1. Play transcript-aligned podcast audio through a service-owned Media3 player
   and expose coherent playback controls through the UI, notification, and lock
   screen.
2. Preserve playback and active-cue state when the Activity detaches and later
   reconnects without creating a second player or losing synchronization.
3. Handle audio focus, transient interruptions, output-route changes, and
   becoming-noisy events according to Android media guidance.
4. Fetch RSS and transcript resources through Kotlin transport, parse them in
   Rust, and persist normalized podcast state in Rust-owned SQLite.
5. Migrate an existing user database from schema v2 to v3 transactionally
   without losing captures, review history, or library state.
6. Let a learner tap a transcript token, resolve it locally, save its sentence
   and padded audio range, and review it through the shared SM-2 policy.
7. Keep the complete v0.2.0 PWA/WASM regression baseline passing while shared
   Rust contracts evolve for Android.

## 3. Users and Core Journeys

### 3.1 Primary Users

- English learners who listen to transcript-enabled podcasts on Android.
- Learners who expect playback to continue reliably with the screen off or
  while using another application.
- Privacy-conscious users who want local lookup, capture, and review state
  without an account or cloud database.

### 3.2 Core Journeys

#### Listen Across Android Lifecycles

1. The learner opens an episode and starts playback.
2. The Activity binds to the playback service and renders the service's current
   episode, position, playback state, and Rust-selected active cues.
3. The learner backgrounds the app or locks the device.
4. Playback and synchronization continue under `MediaSessionService` ownership.
5. Notification, lock-screen, headset, and Bluetooth actions control the same
   player instance.
6. When the learner returns, Compose immediately renders the current service
   state and resumes cue observation without a discontinuity.

#### Add and Open a Podcast

1. The learner enters an RSS 2.0 feed URL.
2. Kotlin performs the network request and passes the response bytes and final
   source URL through `ensub-uniffi`.
3. Rust discovers enclosure and transcript resources and returns normalized,
   typed records.
4. Kotlin fetches a selected WebVTT or SRT resource and passes its bytes and
   source metadata to Rust.
5. Rust validates and persists the feed, episode, and transcript data in
   SQLite before the episode is shown as locally available metadata.

#### Look Up, Capture, and Review

1. The learner taps a Rust-produced token span in the Compose transcript.
2. Rust normalizes the token and resolves its local lexicon entry.
3. One explicit capture action saves the word, sentence, episode provenance,
   cue provenance, and deterministic padded audio slice.
4. The due queue reads the shared Rust scheduling state.
5. The learner reveals the answer, replays the saved slice through Media3, and
   submits a validated 0-5 rating.
6. Rust applies and persists the shared SM-2 transition atomically.

## 4. Delivery Slices

### 4.1 Spike 1 - Delivered Baseline

Spike 1 is complete and is not active feature scope. It established:

- a Kotlin and Jetpack Compose Android project under `android/`;
- a Rust `bindings/ensub-uniffi` facade that generates Kotlin bindings;
- Media3 playback inside the Activity for the bundled demo fixture;
- Rust-owned cue selection exposed to Compose;
- native builds for `arm64-v8a` and `x86_64`;
- automated Rust, Kotlin, lint, APK, and Android-test APK verification; and
- physical verification on a OnePlus Ace Pro (`arm64-v8a`), including playback,
  seeking, overlapping cues at 0:29, and restrained gap anchoring at 0:39.

The baseline proves the FFI, native-link, media, and synchronization path. Its
in-Activity player is intentionally replaced by service ownership in Spike 2.

### 4.2 Spike 2 - Active Target: Background Playback

Spike 2 turns the proof into an Android-correct playback subsystem.

#### Requirements

- A `MediaSessionService` owns the sole Media3 player, media session, current
  episode, playback position, and playback queue state.
- Compose connects through a lifecycle-aware controller and never constructs a
  competing player instance.
- A system media notification and lock-screen surface expose playback state,
  play/pause, seeking, and available episode navigation consistently.
- Media button, headset, Bluetooth, notification, lock-screen, and in-app
  commands converge on the same media session.
- Audio focus gain and loss are explicit. Transient duck requests reduce volume;
  call-like or non-duckable interruptions pause playback; automatic resume is
  allowed only when Ensub itself paused for a transient focus loss.
- `AudioManager.ACTION_AUDIO_BECOMING_NOISY` pauses playback before audio can
  unexpectedly move from a disconnected private route to device speakers.
- Detaching, recreating, or reattaching the Activity does not reset playback.
  The newly attached UI receives a current snapshot and re-evaluates active
  cues through Rust before rendering synchronized transcript state.
- Seeks from any control surface update the first rendered cue state after the
  seek. Overlaps and gaps preserve the delivered Rust semantics.
- Service and controller errors are surfaced as typed UI states without
  terminating the process or leaving a stale notification.

#### Exit Criteria

- Playback remains controllable after backgrounding and screen lock.
- Notification and lock-screen metadata and controls agree with in-app state.
- Focus duck, transient pause/resume, permanent loss, and incoming-call paths
  are covered by deterministic state tests and physical-device checks.
- A becoming-noisy broadcast pauses an actively playing episode.
- Activity recreation and repeated detach/reattach cycles retain episode,
  position, play state, and active-cue synchronization.
- Android unit tests, lint, APK assembly, instrumentation APK assembly, and the
  repository Android CI job pass.

### 4.3 Spike 3 - Data and Ingestion

Spike 3 replaces the bundled-fixture-only data path with durable local podcast
state and real network ingestion.

#### Requirements

- `ensub-sqlite` remains the database implementation. Android accesses it only
  through typed `ensub-uniffi` operations and never issues SQL from Kotlin.
- The user database advances from schema v2 to v3 through a bounded,
  transactional migration that preserves all existing word, context, review,
  and history records.
- Schema v3 persists normalized feed identity, episode identity and aliases,
  enclosure metadata, transcript provenance, normalized cues, playback
  progress, and podcast capture context.
- Concurrent or repeated initialization is idempotent. Migration failure rolls
  back cleanly and returns a typed error without exposing a partially upgraded
  database.
- Kotlin owns HTTP transport, connectivity state, redirects, and Android
  network policy. It passes response bytes, final source URLs, content types,
  and transport failures across the mobile facade.
- Rust owns RSS 2.0 parsing, Podcasting 2.0 transcript discovery, deterministic
  episode identity, alias reconciliation, and WebVTT/SRT normalization.
- Unsupported transcript types, malformed feeds, malformed captions, network
  failures, and storage failures remain distinguishable to the UI.
- Previously persisted episode metadata and transcripts are readable without a
  network request. Whole-episode audio download management is not required by
  this slice.

#### Exit Criteria

- Migration tests prove v2 data survives v3 initialization and that failed
  migrations leave the v2 database recoverable.
- RSS, WebVTT, and SRT fixture contracts produce the same normalized records on
  native and WASM adapters where their public behavior overlaps.
- A synthetic transcript-enabled feed can be fetched, parsed, stored, closed,
  reopened, and rendered from local data on Android.
- Kotlin contains no podcast parser, cue parser, SQL statement, schema number,
  or scheduling rule.

### 4.4 Spike 4 - Immersion Loop

Spike 4 completes the native Android learning loop over the service and storage
foundations.

#### Requirements

- Rust returns token spans suitable for selecting the original Compose text,
  including punctuation and non-ASCII cases, without Kotlin tokenization.
- One tap opens local lemma, pronunciation, part-of-speech, and ranked
  definition results without a network request.
- One explicit confirmation captures the selected word with sentence context,
  feed and episode identity, transcript and cue provenance, playback position,
  and a deterministic Rust-computed padded audio slice.
- Multi-cue sentences use their earliest cue start and latest cue end. Padding
  clamps at zero and at known media duration, and invalid or overflowing ranges
  return typed errors.
- The in-player review queue exposes due cards from Rust-owned SQLite, supports
  prompt and reveal states, and replays only the saved audio range through the
  service-owned player.
- Ratings are restricted to integers from 0 through 5. Rust computes and
  persists the shared SM-2 transition and advances the queue atomically.
- Missing or unreachable audio degrades a review card to saved text while
  preserving the ability to rate it and protecting stored review state.

#### Exit Criteria

- Lookup and capture work after network access is removed.
- Capture round trips preserve structured podcast and audio-slice provenance.
- Review tests prove audio-range stopping behavior and shared SM-2 scheduling
  outcomes for all allowed ratings.
- The complete add, listen, lookup, capture, close, reopen, and review journey
  passes on a physical `arm64-v8a` device.

## 5. Ownership Boundaries

| Capability | Rust ownership | Kotlin / Android ownership |
|---|---|---|
| Podcast and transcript data | RSS 2.0 parsing, transcript discovery, WebVTT/SRT parsing, normalization, validation, identity and alias policy | HTTP transport, redirects, connectivity, Android network policy, user-facing error presentation |
| Synchronization | Cue model, indexed cue selection, overlap and gap semantics, time-range math | Media3 position observation, render cadence, scrolling, Compose presentation |
| Language | Token spans, normalization, morphology, lexicon lookup, ranked local results | Tap handling, panels, focus, accessibility, presentation state |
| Capture and review | Context construction, padded slices, SM-2 policy, rating validation, queue and transition rules | Explicit commands, review UI, Media3 slice playback |
| Persistence | SQLite schema, migrations, transactions, queries, durable podcast and learning records | Database path selection, lifecycle calls through UniFFI, platform backup policy |
| Playback | Portable media identifiers and logical ranges only | Media3 player, `MediaSessionService`, controller, audio focus, notification, lock screen, route events |
| FFI contract | `ensub-uniffi` facade, owned DTOs, typed errors, stable coarse-grained operations | Generated binding consumption and mapping into immutable UI state |

`core_engine` and `language_engine` remain platform-independent and cannot
depend on UniFFI, JNI, Kotlin, Android, Media3, SQLite drivers, UI crates, or an
async runtime. `ensub-uniffi` and `ensub-sqlite` are adapters over those cores.

## 6. Mobile Facade Contract

`bindings/ensub-uniffi` is the only supported Rust entry point for the Android
application. It may compose portable core and native storage capabilities, but
it must remain a thin facade rather than a second domain layer.

The facade must:

- expose owned, versionable DTOs using mobile-safe integer and string types;
- use typed errors with stable categories and actionable detail;
- prefer session or repository objects for related operations rather than
  exposing many chatty calls across FFI;
- keep file paths, SQL, Rust serialization layouts, and internal IDs opaque;
- avoid callbacks on high-frequency playback ticks when Kotlin can request cue
  state at its own render cadence; and
- preserve deterministic behavior across native and WASM adapters when both
  expose the same portable rule.

Generated Kotlin and native libraries remain build outputs. Source Rust,
facade configuration, Gradle integration, and generation scripts remain
reviewable repository inputs.

## 7. Playback and Synchronization Policy

The service is the source of truth for Android playback. The Activity and
Compose hierarchy are replaceable views over a `MediaController`; they do not
own player lifetime.

Media3 supplies current position and discontinuity events. Kotlin passes a
non-negative millisecond position to the Rust transcript session, which returns
all active cue indices and the anchor state. Kotlin renders all simultaneous
active cues, follows the first active cue during overlap, and retains the
restrained preceding anchor during gaps without presenting it as active.

Periodic updates must be suspended when no UI needs transcript rendering, but
service playback and media-session state must continue. Reattachment triggers
an immediate position read and Rust synchronization before periodic observation
resumes. Battery use must not depend on a continuously running Activity timer
while the UI is detached.

## 8. Storage and Migration Policy

Rust owns the SQLite connection, schema version, migrations, transactions, and
domain mapping. Kotlin supplies an application-private database path and calls
coarse-grained facade operations.

The v2 to v3 migration must:

- run in one transaction after connection busy handling is configured;
- preserve every v2 record and identifier;
- create podcast tables, indexes, constraints, and foreign keys deterministically;
- advance `user_version` only after all migration statements succeed;
- remain safe when opening an already migrated database; and
- reject a database newer than the supported schema with a typed error.

Migration tests use synthetic data only. Neither application logs nor errors
may include transcript contents, capture text, private feed credentials, or
database contents.

## 9. Offline, Privacy, and Security

- Local metadata, transcript data, lookup results, captures, and review state
  are stored in the application's private storage.
- Feed and transcript network access is explicit and uses Kotlin transport;
  Rust receives only the bytes and source metadata necessary to parse them.
- Authenticated or DRM-protected feeds are outside v0.3.0 scope.
- No account, analytics service, cloud synchronization, or production remote
  database is required.
- Logs and crash messages must not contain feed credentials, full private URLs,
  transcript bodies, captures, database contents, or generated secrets.
- UniFFI inputs are validated in Rust before they affect domain state or SQL.

## 10. PWA Regression Invariant

The v0.2.0 PWA/Web Player and `ensub-wasm` remain frozen, buildable regression
anchors under the policy in [Platform Status](../docs/platform-status.md).

Android work must not delete, skip, weaken, or replace existing WASM, PWA unit,
browser, distribution, offline, or storage-migration tests. A shared contract
change may update an assertion only when the former behavior is intentionally
invalidated, equivalent coverage remains, and the contract change is recorded.

No Milestone 6 acceptance criterion requires adding a new browser client
feature. Browser changes are limited to maintenance, compatibility, security,
and regression work needed to preserve the v0.2.0 reference behavior.

## 11. Non-Functional Requirements

### 11.1 Reliability

- Normal Activity recreation, backgrounding, and controller reconnection must
  not create multiple players or lose persisted learning state.
- Service commands, FFI calls, parser failures, and storage failures return
  controlled states rather than panics or process termination.
- Writes that span word, context, podcast provenance, or review transition
  records are atomic.

### 11.2 Performance and Battery

- Cue selection remains indexed or equivalently bounded and must not linearly
  scan a full transcript on each playback update.
- Playback state updates do not recompose the entire transcript unnecessarily.
- No high-frequency UI synchronization loop remains active while the Activity
  is detached.
- Network and database work do not run on the Compose main thread.

### 11.3 Accessibility

- Player, notification, lookup, capture, and review controls expose meaningful
  labels, roles, enabled state, and focus order.
- Active cues are not communicated by color alone.
- Text scaling does not hide playback, lookup, capture, or rating controls.

### 11.4 Testability

- Portable policies use fixture-driven or table-driven Rust tests.
- UniFFI DTOs and errors have Rust and Kotlin contract coverage.
- Media session, controller mapping, focus transitions, noisy-route behavior,
  and UI reconnection have deterministic Kotlin tests where framework seams
  permit them.
- Android lint, unit tests, APK assembly, and instrumentation APK assembly run
  in CI; physical-device checks provide release evidence for behaviors that
  cannot be established by build-only CI.

## 12. Acceptance Criteria

Milestone 6 is accepted only when all of the following are true:

1. Spike 1 remains green for both supported ABIs and its physical-device cue
   overlap and gap behavior remains unchanged.
2. A `MediaSessionService` owns the only active Media3 player and playback
   continues when the Activity is backgrounded or destroyed.
3. In-app, notification, lock-screen, headset, and Bluetooth controls reflect
   and manipulate one coherent media-session state.
4. Audio-focus duck, pause, conditional resume, permanent loss, incoming-call,
   and becoming-noisy paths follow the Spike 2 policy.
5. A recreated or reattached UI renders the current episode, playback state,
   position, and Rust-selected active cues without resetting playback.
6. A synthetic schema-v2 database migrates to v3 without losing existing word,
   context, review, history, or library state; a forced migration failure rolls
   back to a recoverable v2 database.
7. Kotlin fetches a synthetic RSS 2.0 feed and transcript while Rust discovers,
   parses, normalizes, and persists the podcast records.
8. Persisted feed, episode, transcript, and playback metadata can be reopened
   and rendered without refetching those resources.
9. Tapping a Rust-produced token span returns local lexicon results without a
   network request, including punctuation and non-ASCII fixture coverage.
10. One explicit action stores a contextual capture with stable episode,
    transcript, cue, sentence, playback-position, and padded-audio provenance.
11. A due podcast card replays only its saved range, accepts a 0-5 rating, and
    persists the shared SM-2 transition without leaving the player.
12. Missing audio degrades review to text without preventing rating or
    corrupting scheduling state.
13. The end-to-end immersion journey passes on a physical `arm64-v8a` device.
14. Android CI and all applicable workspace, WASM, PWA, storage, and secret
    verification suites pass from a clean checkout.
15. No Android-specific UI, lifecycle, transport, notification, audio-focus,
    or database-driver dependency enters `core_engine` or `language_engine`.

## 13. Out of Scope for v0.3.0

- iOS, Kotlin Multiplatform, or a second native mobile client;
- Google Play publication, alternative-store publication, or a committed
  direct-download distribution channel;
- cloud accounts, multi-device sync, hosted learning storage, or cloud backup;
- authenticated or DRM-protected podcast feeds;
- automatic speech recognition or transcript generation;
- whole-episode offline audio download management, transcoding, waveform
  editing, or clip-file export;
- automatic or background LLM analysis;
- new PWA/Web Player client features; and
- new desktop, applet, TUI, CLI, or browser-extension product workflows.

## 14. Delivery Order

1. **M6.1 / Spike 1 - Delivered baseline:** preserve the verified Compose,
   UniFFI, Media3, cue synchronization, ABI, and hardware foundation.
2. **M6.2 / Spike 2 - Active target:** move playback into
   `MediaSessionService` and complete Android lifecycle and system integration.
3. **M6.3 / Spike 3 - Data and ingestion:** ship SQLite v3, the mobile storage
   facade, and real RSS/transcript ingestion.
4. **M6.4 / Spike 4 - Immersion loop:** complete lookup, contextual capture,
   audio-slice playback, and in-player SM-2 review.
5. **M6.5 - Release hardening:** close accessibility, performance, battery,
   migration, privacy, physical-device, and regression evidence for v0.3.0.

Each slice must leave Android buildable, the Rust cores independently testable,
and the PWA/WASM reference baseline green.

## 15. Release Gates

Before v0.3.0 is considered complete:

- every acceptance criterion in this PRD passes with committed automated or
  documented physical-device evidence;
- Rust formatting, workspace check, warning-free Clippy, workspace tests, and
  dependency-boundary inspections pass;
- Android UniFFI generation, both ABI builds, Kotlin unit tests, Android lint,
  debug and release APK assembly, and instrumentation APK assembly pass from a
  clean checkout;
- Spike 2 and the complete immersion journey pass on a physical `arm64-v8a`
  device, including screen-lock, focus-interruption, route-change, and Activity
  reconnection cases;
- schema v2 to v3 forward migration, idempotent reopen, rollback recovery, and
  newer-schema rejection are tested;
- the complete WASM and PWA regression suite required by
  `docs/platform-status.md` remains green;
- privacy and user documentation accurately describe network transport, local
  storage, media behavior, offline limits, and distribution status; and
- the repository secret scanner passes with synthetic fixtures and no
  credentials, private feed data, personal information, or production data in
  source, tests, assets, logs, screenshots, or artifacts.
