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
- The merged Android manifest declares
  `android.permission.FOREGROUND_SERVICE`, declares
  `android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK` for API 34 and newer,
  declares `android.permission.POST_NOTIFICATIONS` for API 33 and newer, and
  registers `dev.ensub.android.player.EnsubMediaSessionService` with
  `android:foregroundServiceType="mediaPlayback"`, `android:exported="true"`,
  and the `androidx.media3.session.MediaSessionService` intent action. These are
  build- and manifest-test invariants rather than runtime feature flags.
- Because the service is exported for platform and trusted-controller
  discovery, its `MediaSession.Callback.onConnect` accepts standard transport
  commands only from the same application or a trusted controller. App-specific
  custom commands are same-UID only, and every other controller is rejected
  without exposing file paths, source URLs, transcript data, or storage calls.
- Playback that promotes the service to the foreground originates from an
  allowed user or MediaSession action, supplies the required media notification,
  and calls foreground promotion within the Android platform deadline. Ensub
  does not use an unrelated foreground-service type to bypass start limits.
- On API 33 and newer, playback and service startup never depend on a
  `POST_NOTIFICATIONS` grant. If Ensub requests that permission for notification
  behavior beyond the exempt media session, it does so from an explicit UI
  action, records denial without repeatedly prompting, and continues playback
  without an exception, restart loop, or degraded transport controls.
- Media button, headset, Bluetooth, notification, lock-screen, and in-app
  commands converge on the same media session.
- Media3 is the sole audio-focus owner. The player uses media/speech audio
  attributes with `handleAudioFocus = true`; Ensub must not make a second
  `AudioManager.requestAudioFocus` request, install a competing focus listener,
  or apply its own duck-volume multiplier. Because podcast audio is speech,
  transient duck and call-like requests suppress playback rather than allowing
  spoken content to continue inaudibly. Resume remains conditional on the
  player's retained `playWhenReady` intent.
- `AudioManager.ACTION_AUDIO_BECOMING_NOISY` pauses playback before audio can
  unexpectedly move from a disconnected private route to device speakers.
- Detaching, recreating, or reattaching the Activity does not reset playback.
  The newly attached UI receives a current snapshot and re-evaluates active
  cues through Rust before rendering synchronized transcript state.
- Seeks from any control surface update the first rendered cue state after the
  seek. Overlaps and gaps preserve the delivered Rust semantics.
- Service and controller errors are surfaced as typed UI states without
  terminating the process or leaving a stale notification.

#### Deterministic Test Contract

Audio-focus tests assert observable Media3 and session state rather than
reimplementing Android focus callbacks in application policy. JVM tests map
immutable `Player` snapshots into service/UI state. Connected instrumentation
tests create a competing focus owner and verify Media3's transitions with the
configured speech attributes. Physical-device checks cover system surfaces and
interruptions that an emulator cannot reproduce faithfully.

The required focus matrix is:

| Initial condition | Focus or route event | Required observable result | Minimum verification |
|---|---|---|---|
| Playing with focus | `AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK` or `AUDIOFOCUS_LOSS_TRANSIENT` | Media3 retains play intent but suppresses speech playback; `isPlaying = false`, progress polling stops, and system surfaces must not claim playback | JVM snapshot-mapper test, connected competing-focus test, and physical duck/call check |
| Suppressed with retained play intent | `AUDIOFOCUS_GAIN` with no intervening user command | Suppression clears and Media3 resumes once from the current position | Connected competing-focus test |
| Suppressed with retained play intent | User pause, stop, or episode change before gain | The user command clears or replaces play intent; later gain must not restart the prior item | Connected competing-focus test |
| Any playing or suppressed state | `AUDIOFOCUS_LOSS` | Media3 clears play intent, reports not playing, and never auto-resumes on a later gain | Connected competing-focus test |
| Playing | `ACTION_AUDIO_BECOMING_NOISY` | Ensub issues pause through the player and clears play intent | JVM receiver-command test, connected broadcast test, and physical wired/Bluetooth route check |
| Paused without focus | Focus request is delayed or denied | Media3 1.7.1 treats every non-granted result as do-not-play: remain paused, clear pending play intent, and expose a recoverable focus-unavailable state | Connected API 26 and API 37 test |

Media3 1.7.1 does not opt into delayed focus gain, so Ensub has no application-
level pending-focus state and must not start on a later unsolicited gain. A
future Media3 upgrade that accepts delayed gain is a playback-contract change
and requires an updated matrix and connected tests before adoption.

The player-to-session mapping is also a fixed contract:

| Media3 source state | Controller and system-surface state | Required side effect | Minimum verification |
|---|---|---|---|
| No current media item | Empty timeline, no stale metadata, `isPlaying = false` | Do not retain a stale media notification | JVM mapper test and connected instrumentation test |
| Current item in `STATE_IDLE` | Current metadata with idle/not-playing state | Await prepare or an explicit play command | JVM mapper test |
| `STATE_BUFFERING` | Buffering and `isPlaying = false`, preserving current metadata and position | Keep the session controllable without claiming playback has advanced | JVM mapper test and connected instrumentation test |
| `STATE_READY` with `playWhenReady = false` | Paused and `isPlaying = false` | Preserve resumable position | JVM mapper test and connected instrumentation test |
| `STATE_READY` with `playWhenReady = true` and suppression reason none | Playing and `isPlaying = true` | Publish current commands, metadata, and position | JVM mapper test, connected instrumentation test, and physical notification/lock-screen check |
| `STATE_READY` with `playWhenReady = true` and a non-none suppression reason | Suppressed/waiting and `isPlaying = false` | Preserve play intent and position, stop progress polling, and do not advertise active playback | JVM mapper test and connected transient-focus test |
| `STATE_ENDED` | Ended and `isPlaying = false` | Mark completion, flush final progress, and do not auto-restart | JVM mapper test and connected instrumentation test |
| Player error | Error/not-playing with the last valid item identity and typed cause | Stop advancing progress, clear stale play state, and expose retry or dismissal | JVM mapper test and connected instrumentation test |
| Controller disconnect and reconnect while the service lives | Service/player state remains unchanged; the new controller receives the current timeline, metadata, commands, position, and play state | Do not create a player or infer a user pause from disconnection | Connected instrumentation test |

Media-session tests must additionally prove that two controllers observe and
command the same player instance, commands and metadata agree across Compose
and system surfaces, seek discontinuities trigger immediate Rust cue
synchronization, Activity recreation does not create another player, and a
reconnected controller receives the authoritative service snapshot before
periodic UI updates resume. Connected tests run on API 26 and API 37; the
OnePlus Ace Pro remains the `arm64-v8a` physical reference for lock-screen,
notification, call, headset, and Bluetooth checks.

Manifest tests inspect the merged manifest for all three permissions, the exact
service component, exported policy, Media3 intent action, and `mediaPlayback`
service type. Controller tests prove same-app and trusted standard transport,
same-UID custom-command access, and rejection of untrusted controllers.
Connected API 33 and API 37 tests exercise both granted and denied notification-
permission states. Denial must not prevent a user-initiated play command,
foreground promotion, controller reconnection, or service-owned playback;
tests assert the system surfaces Android makes available in that permission
state rather than inventing an app-owned fallback notification.

#### Exit Criteria

- Playback remains controllable after backgrounding and screen lock.
- Notification and lock-screen metadata and controls agree with in-app state.
- Speech handling for duck requests, denied focus, transient suppression and
  resume, permanent loss, and incoming-call paths is covered by the required
  mapper, connected, and physical-device checks.
- A becoming-noisy broadcast pauses an actively playing episode.
- Activity recreation and repeated detach/reattach cycles retain episode,
  position, play state, and active-cue synchronization.
- The merged manifest and API 33+ permission-state tests satisfy the foreground-
  service and notification-denial contract.
- A paused and fully idle service tears down after the bounded policy in
  Section 7.2. Active, buffering, and focus-suppressed states remain alive while
  the process survives; a degraded persistence retry cannot prevent eventual
  idle teardown.
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
  progress and completion state, and podcast capture context.
- Concurrent or repeated initialization is idempotent. Migration failure rolls
  back cleanly and returns a typed error without exposing a partially upgraded
  database.
- Kotlin owns HTTP transport, connectivity state, redirects, and Android
  network policy. It passes successful response bytes, final source URLs, and
  content types across the mobile facade. Transport failures remain Kotlin-
  owned and are not reclassified as Rust parsing failures.
- Rust owns RSS 2.0 parsing, Podcasting 2.0 transcript discovery, deterministic
  episode identity, alias reconciliation, and WebVTT/SRT normalization.
- Unsupported transcript types, malformed feeds, malformed captions, network
  failures, and storage failures remain distinguishable to the UI.
- Previously persisted episode metadata and transcripts are readable without a
  network request. Whole-episode audio download management is not required by
  this slice.
- Kotlin owns one process-wide, app-private Media3 `SimpleCache`, exposed to
  players through `CacheDataSource` and bounded by a 256 MiB least-recently-used
  evictor under `context.cacheDir/ensub-media-v1`. Its `CacheKeyFactory` uses an
  opaque SHA-256 digest of stable episode identity plus a Rust-issued media-
  resource revision derived from the selected enclosure generation and source
  metadata supplied through the facade. Kotlin supplies newly observed `ETag`
  or `Last-Modified` validators to the revision operation. Raw URLs never appear
  in cache keys; any enclosure alias, representation, or validator change
  produces a new revision and invalidates the older cache generation.
- A cache miss may cause the upstream Media3 data source to issue an HTTP range
  request. A feed or enclosure is not rejected merely because its server omits
  `Accept-Ranges`, ignores a range, or returns `200` instead of `206`. Ordinary
  episode listening may continue sequential streaming, but a review-only load
  that receives `200` for a nonzero range aborts before more than 2 MiB of pre-
  slice bytes are transferred and falls back to saved text. Ensub never starts
  an unbounded sequential transfer or materializes a clip file solely to make a
  short review slice available.
- Cached audio is disposable transport data, not durable capture state. It is
  stored only in the platform non-backup `cacheDir`, can be cleared without
  losing podcast metadata or learning history, and is released with the process-
  wide cache owner rather than opened independently by each player.

#### Network Retry Contract

Every request has exactly one retry owner. The Kotlin ingestion client owns
retries for idempotent feed and transcript `GET` requests, and its HTTP client's
automatic connection retry is disabled. A custom Media3
`LoadErrorHandlingPolicy` is the sole retry owner for enclosure and cache-miss
loads, and its upstream HTTP stack is configured for one attempt per load.
Each operation is limited to three total attempts: the initial request and at
most two retries. Retryable failures are connection resets, timeouts, and HTTP
`408`, `429`, `500`, `502`, `503`, and `504`. Other `4xx` responses, certificate
or hostname failures, malformed redirect targets, and policy rejections fail
without an automatic retry.

Without `Retry-After`, retries use exponential delays of 500 milliseconds and
one second with full jitter from zero through the selected delay. A valid
`Retry-After` replaces that delay but is capped at 30 seconds. Cancellation
interrupts requests and pending delays immediately. A known-offline state
returns a typed offline result without running a background retry loop; a later
user action starts a new bounded operation. Rust parser, validation, migration,
and storage failures never enter the HTTP retry loop.

#### Exit Criteria

- Migration tests prove v2 data survives v3 initialization and that failed
  migrations leave the v2 database recoverable.
- RSS, WebVTT, and SRT fixture contracts produce the same normalized records on
  native and WASM adapters where their public behavior overlaps.
- A synthetic transcript-enabled feed can be fetched, parsed, stored, closed,
  reopened, and rendered from local data on Android.
- Fake-transport and fake-clock tests prove the retryable status set, three-
  attempt bound, jitter bounds, `Retry-After` cap, cancellation, offline result,
  and exclusion of parser and storage failures from transport retries. Fake
  HTTP-server request counts, rather than wrapper counters alone, prove that
  ingestion and Media3 do not multiply attempts across retry layers.
- Cache tests prove the 256 MiB LRU bound, stable redacted cache keys, offline
  cache hits, source-revision invalidation, eviction-safe fallback, the 2 MiB
  review-miss transfer bound, `cacheDir` placement, and cache clearing without
  deletion of durable Rust-owned state.
- Merged-manifest, extraction-rule, and cache-path tests prove the configured
  defense-in-depth policy for SQLite, captures, transcripts, lexicon assets,
  and media-cache bytes before production storage is accepted; they do not
  claim to certify every OEM transfer implementation.
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
  persists the shared SM-2 state-and-history transition atomically; Kotlin
  advances its derived presentation queue only after that commit succeeds.
- Missing or unreachable audio degrades a review card to saved text while
  preserving the ability to rate it and protecting stored review state.
- A cache hit replays the saved range without network access. On a cache miss,
  Media3 may request the relevant upstream bytes; if seeking is unsupported,
  the bytes were evicted, or the device is offline, review remains text-capable
  and rateable without downloading the whole episode or creating a clip file.

#### Review Queue and Auto-Advance Contract

Opening review captures one immutable UTC `as_of` timestamp and loads batches
of at most 20 cards ordered by `next_review_at ASC`, then stable `word_id ASC`.
Every reload in that review session uses the original cutoff, so cards that
become due after the session opens do not appear until a new session.

A rating disables duplicate submission and advances only after the facade
reports the successful atomic state-and-history commit corresponding to core
`ReviewUpdate::Updated`. Advancement cancels the outgoing snippet and presents
the next card in prompt state; it does not auto-reveal, auto-play audio, or
synthesize a rating. A storage or FFI failure leaves the current card revealed
and retryable without advancing. The facade exports core
`ReviewUpdate::Conflict` as `MobileErrorCategory::ReviewConflict`; Android then
discards the stale submitted transition, reloads using the original cutoff,
and never replays the rating automatically.

When the current batch is exhausted, Android reloads with the same cutoff. An
empty reload completes the session; otherwise the first returned card becomes
the prompt. Context selection, snippet availability, snippet replay, and cache
state never change queue ordering, eligibility, or scheduling.

#### Exit Criteria

- Lookup and capture work after network access is removed.
- Capture round trips preserve structured podcast and audio-slice provenance.
- Review tests prove audio-range stopping behavior and shared SM-2 scheduling
  outcomes for all allowed ratings.
- Queue tests prove the fixed cutoff and tie-break order, successful-only
  advancement, duplicate-submit suppression, failure retention, conflict
  refresh, batch reload, empty completion, and absence of automatic reveal or
  snippet playback.
- Cached slices replay with network disabled; missing or evicted bytes preserve
  text review, rating, and scheduling behavior.
- TalkBack, Compose semantics, deterministic focus traversal, 200 percent font
  scaling, and non-color-only state pass the accessibility contract in Section
  12.3 on an emulator and the physical reference device.
- The complete add, listen, lookup, capture, close, reopen, and review journey
  passes on a physical `arm64-v8a` device.

## 5. Ownership Boundaries

| Capability | Rust ownership | Kotlin / Android ownership |
|---|---|---|
| Podcast and transcript data | RSS 2.0 parsing, transcript discovery, WebVTT/SRT parsing, normalization, validation, identity and alias policy, media-resource revision | HTTP transport, redirects, response validators, connectivity, Android network policy, user-facing error presentation |
| Synchronization | Cue model, indexed cue selection, overlap and gap semantics, time-range math | Media3 position observation, render cadence, scrolling, Compose presentation |
| Language | Token spans, normalization, morphology, lexicon lookup, ranked local results | Tap handling, panels, focus, accessibility, presentation state |
| Capture and review | Context construction, padded slices, SM-2 policy, rating validation, queue and transition rules | Explicit commands, review UI, Media3 slice playback |
| Persistence | SQLite schema, migrations, transactions, queries, durable podcast and learning records | Database path selection, lifecycle calls through UniFFI, platform backup policy |
| Playback | Portable media identifiers and logical ranges only | Media3 player, `MediaSessionService`, controller, audio focus, notification, lock screen, route events, upstream data source, and disposable media cache |
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
- use the typed error categories and retry guidance in Appendix A;
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

### 7.1 Playback Progress Persistence

The service keeps the live position in memory and persists progress through a
coalescing writer outside the main thread. During active playback, it schedules
one routine checkpoint attempt after every 10 seconds of monotonic elapsed
time. Only the latest unsent routine position for an episode may be pending,
and routine attempts must never be scheduled more frequently than once per 10
seconds.

A healthy sink accepts each request and completes its durable write within two
seconds. While the sink is healthy, the maximum durable-progress lag during
continuous playback is therefore 12 seconds: the 10-second scheduling interval
plus the two-second writer bound.

The service makes an immediate, non-debounced persistence request after a
completed seek, pause, stop, episode transition, playback end, and orderly
service shutdown. "Immediate" means the current snapshot is enqueued during
handling of that event; it does not mean blocking the player or main thread on
SQLite. The request may supersede an older unsent snapshot for the same episode
but must not wait for the next routine boundary or a timed coalescing window.

An episode transition synchronously captures the outgoing snapshot and
enqueues it ahead of any incoming-episode checkpoint; the two identities must
never be coalesced. A failed outgoing write remains pending under its own
identity and does not block playback of the incoming episode. Activity detach
alone does not request persistence because the service continues to own
playback.

Progress is stored by stable internal episode identity as a clamped integer
millisecond position plus a completion flag. A cold service start restores an
incomplete episode at its last durable position but remains paused until an
explicit play command. A completed episode is marked complete and begins at
zero when the learner explicitly replays it. A rejected request or write that
exceeds two seconds must not stop audio: the service retains the newest
in-memory position, exposes a typed non-fatal durability-degraded state, and
continues routine and event-driven retry attempts. The 12-second durability
bound is suspended from the first reported failure or timeout until a later
write succeeds; that success clears the degraded state and starts a new bound.

Spike 2 implements this cadence against an injectable progress sink and proves
it with a recording test double. Spike 3 supplies the production sink through
the UniFFI storage facade and Rust-owned SQLite; Kotlin never persists progress
through preferences, Room, or direct SQL.

Deterministic tests use a fake monotonic clock and controllable recording store
to prove attempts at each 10-second boundary, routine coalescing, immediate
event enqueues, two-second completion and timeout behavior, episode-transition
ordering, degraded-state entry and recovery, cold restore, and the 12-second
healthy-sink process-death bound.

### 7.2 Idle Service Release

Foreground demotion and service destruction are separate decisions. Media3
updates foreground and notification state as playback changes; Ensub does not
keep foreground priority solely to preserve a paused player. While the process
survives, 15 minutes of continuous idle triggers app-initiated teardown, which
completes within the final two-second writer bound. Android may reclaim the non-
foreground, unbound service or its process earlier; correctness depends on
durable cold restoration, not on receiving the full idle window.

The timer is eligible only while the player is paused, ended, or in a terminal
error state and there is no bound Ensub UI controller, retained play intent, or
active review snippet. Playing, buffering, preparing to play, and focus
suppression with retained play intent are never idle. A controller connection,
MediaSession command, new media item, play request, snippet request, or return
to a non-eligible state cancels and resets the timer. Entering idle cancels
nonessential upstream cache loads. An in-flight progress write retains its
existing two-second writer bound but does not postpone timer start; an unsent or
failed degraded-state retry does not make the service permanently non-idle.

At expiry, the shutdown sequence has one total two-second off-main-thread writer
budget. It enqueues the latest progress snapshot, superseding an older unsent
snapshot for that episode; an already in-flight write and the final snapshot
share the remaining budget. The service then releases the player, media session,
audio-focus and route resources, and `SimpleCache` reference before calling
`stopSelf`, even if the newest snapshot could not be written. An I/O failure
must not turn the idle timeout into an unbounded resident service. A later
command cold-starts one service, restores the last durable episode and position,
and remains paused until an explicit play request.

Fake-clock JVM tests cover timer eligibility, every reset condition, the final
flush bound, release ordering, and failed-flush teardown. A connected test
covers paused UI detachment, 15-minute expiry through an injectable clock,
service removal, one-instance restoration, and absence of autoplay. A separate
process-death test before timer expiry proves the same paused restoration and
does not require Android to preserve the idle service for 15 minutes.

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

## 9. SM-2 Reference Contract

The normative scheduling contract is `ensub-sm2-v1`, implemented only by
`core_engine::schedule_review`. Android, WASM, desktop, CLI, and future adapters
must submit the current state, validated rating, and caller-supplied UTC review
timestamp to Rust; they must not reproduce or adjust the formula.

For a rating `q` in the inclusive range 0 through 5, `ensub-sm2-v1` is:

- a new card starts with ease factor `2.5`, repetitions `0`, interval `0`, no
  last rating, and `next_review_at` equal to its supplied creation timestamp;
- `d = 5 - q` and the new ease factor is
  `max(1.3, old_ease + 0.1 - d * (0.08 + d * 0.02))`;
- if `q < 3`, repetitions reset to `0` and the interval is `1` day;
- if `q >= 3`, repetitions increment with saturation and the interval uses the
  pre-review repetition count: `0 -> 1 day`, `1 -> 6 days`, otherwise the
  previous interval multiplied by the pre-review ease factor and rounded to
  the nearest whole day using Rust `f64::round` semantics (halfway values away
  from zero); the ease factor itself is not decimal-quantized;
- a multiplied interval is clamped to the inclusive range `1..u32::MAX`;
- `next_review_at` is the supplied `reviewed_at` plus the resulting whole-day
  interval, and timestamp overflow returns a typed error; and
- the stored last rating is `q`.

`crates/core_engine/tests/fixtures/sm2-v1.json` is the versioned conformance
fixture to be added in Spike 4. It must contain the initial state and vectors
for ratings 0 through 5, the `1 -> 6 -> rounded` progression, rating-3 use of
the pre-review ease factor, the `1.3` ease floor, interval saturation, equal
input determinism, invalid ratings, and timestamp overflow. The Rust unit suite
must consume every vector. Each exported UniFFI/WASM scheduling boundary must
consume every full-schedule vector and produce identical integer/timestamp
fields or error categories. Ease-factor comparisons use an absolute tolerance
of `1e-12`, matching the existing Rust reference tests, so the contract does
not depend on decimal display formatting.

The saturation case is split deliberately: a helper-level vector proves that
interval calculation clamps to `u32::MAX`; only the Rust helper suite consumes
that vector. A separate full `schedule_review` vector using the saturated input
expects `ReviewDateOverflow` because Chrono cannot represent a timestamp
millions of years in the future; Rust and every exported scheduling boundary
consume that vector. The fixture must not claim that a saturated interval can
produce a successful scheduled state.

Committing a review is a compare-and-swap against the expected current state.
The replacement state and immutable review-history event are persisted in one
transaction; a stale expected state is rejected and refreshed rather than
replaying the transition. Any change to the formula, rounding, pass boundary,
defaults, state fields, or error semantics requires a new contract identifier,
an explicit persisted-state compatibility decision, and updated regression
evidence for every active or frozen adapter.

## 10. Offline, Privacy, and Security

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

### 10.1 Android Backup and Data Extraction

The Android application sets `android:allowBackup="false"` as a tested privacy
invariant. API 31-and-newer `dataExtractionRules` and legacy
`fullBackupContent` rules deny the database, root, files, shared-preferences,
and external-file domains that can contain SQLite, WAL and journal files,
transcripts, captures, extracted lexicon assets, or diagnostics. The Media3
cache is rooted only under `context.cacheDir/ensub-media-v1`, a platform non-
backup cache domain that is not representable as a backup-rule XML domain.
This defense in depth reduces unintended platform or OEM data movement; it does
not rely on assumptions about a provider's encryption or claim control over an
OEM that disregards Android's declared policy.

Merged-manifest tests assert the disabled-backup flag and the applicable rule
resource. Resource tests assert that durable state is covered by explicit
domain exclusions, while a cache-path test asserts that `SimpleCache` cannot be
constructed under `filesDir`, `noBackupFilesDir`, external storage, or another
durable domain. Clearing or recreating the cache must never create, delete, or
mutate durable learning state. These tests establish Ensub's configuration,
not the runtime behavior of every Android-derived OEM backup implementation.

## 11. PWA Regression Invariant

The v0.2.0 PWA/Web Player and `ensub-wasm` remain frozen, buildable regression
anchors under the policy in [Platform Status](../docs/platform-status.md).

Android work must not delete, skip, weaken, or replace existing WASM, PWA unit,
browser, distribution, offline, or storage-migration tests. A shared contract
change may update an assertion only when the former behavior is intentionally
invalidated, equivalent coverage remains, and the contract change is recorded.

No Milestone 6 acceptance criterion requires adding a new browser client
feature. Browser changes are limited to maintenance, compatibility, security,
and regression work needed to preserve the v0.2.0 reference behavior.

## 12. Non-Functional Requirements

### 12.1 Reliability

- Normal Activity recreation, backgrounding, and controller reconnection must
  not create multiple players or lose persisted learning state.
- Service commands, FFI calls, parser failures, and storage failures return
  controlled states rather than panics or process termination.
- Writes that span word, context, podcast provenance, or review transition
  records are atomic.

### 12.2 Performance and Battery

- Cue selection remains indexed or equivalently bounded and must not linearly
  scan a full transcript on each playback update.
- Playback state updates do not recompose the entire transcript unnecessarily.
- No high-frequency UI synchronization loop remains active while the Activity
  is detached.
- Network and database work do not run on the Compose main thread.

### 12.3 Accessibility

- Player, notification, lookup, capture, and review controls expose meaningful
  labels, roles, enabled state, and focus order.
- Active cues are not communicated by color alone.
- TalkBack traversal follows playback, transcript, lookup or review content,
  and primary action order without trapping focus or moving focus on each
  position update.
- Cue synchronization does not publish a per-tick live-region announcement.
  An explicit seek or user-focused cue may announce the resulting cue once;
  ordinary playback highlighting remains visually updated without speech spam.
- At 200 percent font scale, text reflows without hiding, clipping, or
  overlapping playback, lookup, capture, reveal, or rating controls.
- Semantics and screenshot tests run at default and 200 percent font scale;
  TalkBack traversal, system media controls, non-color-only cue state, and cue-
  announcement behavior are verified on an emulator and the physical reference
  device before Spike 4 acceptance.

### 12.4 Testability

- Portable policies use fixture-driven or table-driven Rust tests.
- UniFFI DTOs and errors have Rust and Kotlin contract coverage, including every
  category-precedence vector in Appendix A.
- Player-to-session mapping is covered by exhaustive JVM snapshot vectors;
  Media3 focus ownership, denied/transient/permanent focus behavior, service,
  controller, noisy-route, and UI reconnection are covered by connected tests
  on API 26 and API 37.
- Physical-device evidence covers notification, lock-screen, incoming-call,
  wired-headset, Bluetooth-route, and detached-UI behavior on `arm64-v8a`.
- Playback-progress cadence uses a fake clock and recording store to prove
  10-second attempts, immediate event requests, two-second writer behavior,
  the healthy-sink durability bound, degraded-state recovery, and cold restore.
- The Rust suite consumes every `ensub-sm2-v1` conformance vector; every
  exported scheduling boundary consumes the full-schedule subset without
  platform-specific expectations.
- Android lint, unit tests, APK assembly, and instrumentation APK assembly run
  in CI; physical-device checks provide release evidence for behaviors that
  cannot be established by build-only CI.

### 12.5 Release Integrity and Signing

- M6.5 selects and documents an Android application ID, signing identity,
  external secret provider, signer access policy, rotation procedure, and
  recovery owner before v0.3.0 acceptance, without selecting a distribution
  channel.
- Private signing keys, keystore files, passwords, and provider credentials
  never enter source control, fixtures, build caches, CI artifacts, screenshots,
  or logs. Release signing is injected only from the approved external provider;
  the public signer certificate and fingerprint may be retained for verification.
- Routine CI outputs are debug-signed or unsigned and are labeled
  non-distributable. The release-hardening gate produces a signed synthetic-
  data release candidate and verifies its application ID, version, signer
  certificate fingerprint, and absence of debug signing without disclosing
  private material.

## 13. Acceptance Criteria

Milestone 6 is accepted only when all of the following are true:

1. Spike 1 remains green for both supported ABIs and its physical-device cue
   overlap and gap behavior remains unchanged.
2. A `MediaSessionService` owns the only active Media3 player and playback
   continues when the Activity is backgrounded or destroyed.
3. The merged manifest declares the required media-playback foreground-service
   permissions, exact service component, action, exported policy, and type;
   controller authorization passes and API 33+ notification denial neither
   crashes nor blocks service-owned playback or transport commands.
4. In-app, notification, lock-screen, headset, and Bluetooth controls reflect
   and manipulate one coherent media-session state.
5. Speech suppression for duck requests, denied focus, conditional resume,
   permanent loss, incoming-call, and becoming-noisy paths follow the Spike 2
   policy.
6. A recreated or reattached UI renders the current episode, playback state,
   position, and Rust-selected active cues without resetting playback.
7. The player-to-session matrix passes as JVM snapshot tests, the focus and
   service matrix passes as connected API 26 and API 37 tests, and the listed
   notification, lock-screen, call, headset, and Bluetooth cases pass on the
   physical reference device.
8. A fully idle paused service initiates teardown at 15 minutes and completes
   within the bounded two-second final progress request if the process survives.
   Earlier Android process death and timer-driven teardown both restore one
   paused service; a failed progress retry cannot keep it resident indefinitely.
9. Active playback schedules one routine persistence attempt per 10 seconds,
   event-driven requests bypass debounce, and a healthy sink completes each
   write within two seconds. The documented 12-second durability bound holds
   while healthy and is visibly suspended after failure until a retry succeeds.
10. A synthetic schema-v2 database migrates to v3 without losing existing word,
    context, review, history, or library state; a forced migration failure rolls
    back to a recoverable v2 database.
11. Kotlin fetches a synthetic RSS 2.0 feed and transcript while Rust discovers,
    parses, normalizes, and persists the podcast records; transport retries obey
    the bounded status, backoff, cancellation, and offline policy.
12. The 256 MiB app-private Media3 cache serves an offline cache hit, evicts by
    LRU, invalidates changed enclosure revisions, uses redacted stable keys,
    bounds a range-ignoring review miss to 2 MiB of pre-slice transfer, and
    clears without altering durable state.
13. Persisted feed, episode, transcript, and playback metadata can be reopened
   and rendered without refetching those resources.
14. Tapping a Rust-produced token span returns local lexicon results without a
   network request, including punctuation and non-ASCII fixture coverage.
15. One explicit action stores a contextual capture with stable episode,
    transcript, cue, sentence, playback-position, and padded-audio provenance.
16. A fixed-cutoff review queue advances only after the facade reports a core
    `ReviewUpdate::Updated`, handles `ReviewConflict` and storage failure without
    replaying a rating, and preserves due-time and word-ID ordering across batch
    reloads.
17. A due podcast card replays only its saved range, accepts a 0-5 rating, and
    persists the `ensub-sm2-v1` transition without leaving the player; every
    native and WASM scheduling boundary passes the full-schedule conformance
    vectors.
18. Cached slices replay offline, while missing, evicted, unreachable, or range-
    unsupported audio degrades review to text without preventing rating or
    corrupting scheduling state.
19. Compose semantics, TalkBack traversal, cue announcements, non-color state,
    and 200 percent text scaling meet the Spike 4 accessibility contract.
20. Merged-manifest, extraction-rule, and cache-path tests establish the backup-
    denial configuration and keep disposable audio under `cacheDir`, without
    overclaiming control of every OEM transfer implementation.
21. Every UniFFI failure exposes generated category, operation, and retry-advice
    values; all Appendix A precedence vectors pass in Rust and Kotlin without
    parsing an error string.
22. The end-to-end immersion journey passes on a physical `arm64-v8a` device.
23. Android CI and all applicable workspace, WASM, PWA, storage, and secret
    verification suites pass from a clean checkout.
24. No Android-specific UI, lifecycle, transport, notification, audio-focus,
    or database-driver dependency enters `core_engine` or `language_engine`.
25. M6.5 proves the externally provisioned signing path with a synthetic-data
    release candidate while routine CI artifacts remain clearly
    non-distributable and no private signing material enters the repository.

## 14. Out of Scope for v0.3.0

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

## 15. Delivery Order

1. **M6.1 / Spike 1 - Delivered baseline:** preserve the verified Compose,
   UniFFI, Media3, cue synchronization, ABI, and hardware foundation.
2. **M6.2 / Spike 2 - Active target:** move playback into
   `MediaSessionService` and complete Android lifecycle and system integration.
3. **M6.3 / Spike 3 - Data and ingestion:** ship SQLite v3, the mobile storage
   facade, bounded transport retry, the private media cache, and real
   RSS/transcript ingestion.
4. **M6.4 / Spike 4 - Immersion loop:** complete lookup, contextual capture,
   audio-slice playback, deterministic review queue behavior, in-player SM-2
   review, and accessibility acceptance.
5. **M6.5 - Release hardening:** close performance, battery, migration,
   privacy, signing, physical-device, and regression evidence for v0.3.0.

Each slice must leave Android buildable, the Rust cores independently testable,
and the PWA/WASM reference baseline green.

## 16. Release Gates

Before v0.3.0 is considered complete:

- every acceptance criterion in this PRD passes with committed automated or
  documented physical-device evidence;
- Rust formatting, workspace check, warning-free Clippy, workspace tests, and
  dependency-boundary inspections pass;
- Android UniFFI generation, both ABI builds, Kotlin unit tests, Android lint,
  debug and release APK assembly, instrumentation APK assembly, and connected
  Spike 2 tests on API 26 and API 37 pass from a clean checkout;
- merged-manifest checks prove the exact foreground-service declaration and
  backup policy, controller tests prove the exported-service authorization
  rules, and connected API 33 and API 37 tests prove notification denial does
  not block playback;
- Spike 2 and the complete immersion journey pass on a physical `arm64-v8a`
  device, including screen-lock, focus-interruption, route-change, and Activity
  reconnection cases;
- playback checkpoint cadence, immediate event requests, two-second sink
  behavior, healthy-sink process-death loss, degraded-state recovery, and cold
  restoration are covered by deterministic fake-clock tests, and the idle-
  release suite proves the 15-minute eligibility and bounded teardown policy;
- transport retry and cache suites prove the request-attempt bound, retry status
  set, single retry owner, backoff and cancellation behavior, 256 MiB LRU limit,
  redacted revision-aware keys, offline hit, eviction fallback, and the 2 MiB
  range-unsupported review fallback;
- all `ensub-sm2-v1` fixture vectors pass through `core_engine`, the
  full-schedule subset passes through UniFFI and WASM, and persisted review
  commits retain compare-and-swap atomicity and the fixed-cutoff queue contract;
- every Appendix A error-precedence vector passes through Rust and generated
  Kotlin mappings with stable `MobileOperation` and `RetryAdvice` values;
- schema v2 to v3 forward migration, idempotent reopen, rollback recovery, and
  newer-schema rejection are tested;
- accessibility evidence covers Compose semantics, TalkBack traversal, cue-
  announcement restraint, non-color state, and 200 percent text scaling;
- a synthetic-data release candidate is externally signed and its application
  ID, version, signer fingerprint, and non-debug identity are verified without
  storing or logging private signing material;
- the complete WASM and PWA regression suite required by
  `docs/platform-status.md` remains green;
- privacy and user documentation accurately describe network transport, local
  storage, media behavior, offline limits, and distribution status; and
- the repository secret scanner passes with synthetic fixtures and no
  credentials, private feed data, personal information, or production data in
  source, tests, assets, logs, screenshots, or artifacts.

## Appendix A - Mobile Facade Error Types

Kotlin control flow depends on a generated `MobileErrorCategory` value and must
never parse an exception message, Rust debug representation, or SQLite source
string. The v0.3.0 category contract is:

| Category | Meaning | Default retry advice |
|---|---|---|
| `InvalidArgument` | The caller supplied a value outside the facade contract | Never retry unchanged input |
| `UnsupportedResource` | The resource kind, media form, feed feature, or transcript type is unsupported | User action or a newer client is required |
| `ParseFailure` | Rust could not validate or normalize supplied feed or transcript bytes | Do not retry the same bytes |
| `NotFound` | A requested stable domain object no longer exists | Refresh the owning screen or session |
| `StaleRevision` | A session, token, or expected revision no longer matches current state | Refresh, then allow an explicit retry |
| `ReviewConflict` | The compare-and-swap review commit lost a race | Refresh the fixed-cutoff queue; never replay the rating |
| `StorageBusy` | SQLite remained busy beyond its bounded wait policy | Retry the same idempotent read or reopen operation with backoff |
| `StorageUnavailable` | The database or required local asset cannot currently be opened | Retry after availability changes |
| `MigrationFailed` | A transactional schema migration rolled back | Do not loop; retain the recoverable prior database and surface recovery |
| `NewerSchema` | The database was created by a newer incompatible Ensub schema | Require a compatible application version |
| `StorageIo` | A filesystem or SQLite I/O operation failed | Follow the attached retry advice; do not assume idempotence |
| `NumericOverflow` | A time, range, counter, or conversion exceeded its supported domain | Never retry unchanged input |
| `InternalInvariant` | Rust rejected an impossible internal state without panicking | Never retry automatically; expose a recoverable generic failure |

Every exported failure carries the category, a generated `MobileOperation`
enum value, and a `RetryAdvice` value from `Never`, `RetryWithBackoff`,
`RefreshThenRetry`, or `UserActionRequired`. Operation names are generated
contract values, never unconstrained strings. An optional safe-detail code may
distinguish documented subcases but is diagnostic rather than user-facing.
Kotlin maps this structured failure into its sealed presentation-error model
and owns localized copy.

Overlapping source failures use this normative mapping:

| Operation context or source condition | Required category |
|---|---|
| Schema version is newer than this client before migration begins | `NewerSchema` |
| Review compare-and-swap returns core `ReviewUpdate::Conflict` | `ReviewConflict` |
| Any other session token or expected revision is stale | `StaleRevision` |
| A migration statement fails after the migration transaction begins and rollback succeeds | `MigrationFailed` |
| SQLite remains busy before a migration transaction begins, or during an ordinary operation, after bounded waiting | `StorageBusy` |
| A required database or local asset cannot be located or opened before a usable handle exists | `StorageUnavailable` |
| A filesystem or database read/write fails after a usable handle exists | `StorageIo` |
| The resource kind is recognized but unsupported | `UnsupportedResource` |
| Bytes claim a supported format but are malformed | `ParseFailure` |
| A stable domain identifier has no record after storage opens successfully | `NotFound` |
| A validated conversion or calculation exceeds its numeric domain | `NumericOverflow` |
| Caller input violates another documented precondition | `InvalidArgument` |
| No documented category applies to an impossible internal state | `InternalInvariant` |

The facade must classify by operation phase, not by matching source-error text.
Rust and generated-Kotlin contract tests cover every mapping row, including
busy-before-migration versus busy-after-transaction-start, unsupported versus
malformed resources, and unavailable-before-open versus I/O-after-open.

Rust source chains, SQL, database paths, transcript or capture text, raw feed
URLs, credentials, response bodies, and database contents never cross UniFFI
as error detail. Kotlin-owned HTTP and Android framework failures remain outside
`MobileErrorCategory`, but the presentation layer may map them beside facade
failures without erasing their transport or platform origin. Adding, removing,
or changing a category, operation, retry meaning, or safe-detail code is a
mobile-facade contract change and requires Rust and generated-Kotlin tests.
