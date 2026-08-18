const $ = (id) => document.getElementById(id);

function formatTime(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const value = Math.floor(seconds);
  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const tail = String(value % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${tail}` : `${minutes}:${tail}`;
}

function resourceLabel(resource, index) {
  const format = resource.format ? resource.format.toUpperCase() : resource.mimeType;
  return [resource.language, format].filter(Boolean).join(" · ") || `Transcript ${index + 1}`;
}

export function createView() {
  const elements = {
    audio: $("audio"), empty: $("empty-state"), workspace: $("episode-workspace"),
    feedForm: $("feed-form"), feedUrl: $("feed-url"), demo: $("demo-button"),
    list: $("episode-list"), count: $("episode-count"), feedTitle: $("feed-title"),
    title: $("episode-title"), artwork: $("episode-artwork"), playerArtwork: $("player-artwork"),
    playerTitle: $("player-title"), playerFeed: $("player-feed"), transcript: $("transcript"),
    transcriptStatus: $("transcript-status"), transcriptSelect: $("transcript-select"),
    follow: $("follow-button"), play: $("play-button"), back: $("back-button"),
    forward: $("forward-button"), seek: $("seek"), elapsed: $("elapsed"), remaining: $("remaining"),
    speed: $("speed"), mute: $("mute-button"), volume: $("volume"), network: $("network-state"),
    inspector: $("lookup-inspector"), lookupSurface: $("lookup-surface"), lookupPhonetic: $("lookup-phonetic"),
    lookupBody: $("lookup-body"), lookupChoice: $("lookup-choice"), lookupChoiceLabel: $("lookup-choice-label"),
    capture: $("capture-button"), lookupClose: $("lookup-close"),
    reviewButton: $("review-button"), reviewDueCount: $("review-due-count"),
    reviewDialog: $("review-dialog"), reviewClose: $("review-close"), reviewProgress: $("review-progress"),
    reviewStatus: $("review-status"), reviewContent: $("review-content"), reviewContext: $("review-context"),
    reviewContextLabel: $("review-context-label"), reviewSentence: $("review-sentence"), reviewPlay: $("review-play"),
    reviewAudioStatus: $("review-audio-status"), reviewReveal: $("review-reveal"), reviewAnswer: $("review-answer"),
    reviewLemma: $("review-lemma"), reviewPhonetic: $("review-phonetic"), reviewDefinition: $("review-definition"),
    reviewRatings: $("review-ratings"), reviewNext: $("review-next"),
    disambiguationActions: $("disambiguation-actions"), disambiguate: $("disambiguate-button"),
    providerSettingsButton: $("provider-settings-button"), disambiguationResult: $("disambiguation-result"),
    disambiguationMessage: $("disambiguation-message"), providerSettingsDialog: $("provider-settings-dialog"),
    providerSettingsForm: $("provider-settings-form"), providerEndpoint: $("provider-endpoint"),
    providerModel: $("provider-model"), providerCredential: $("provider-credential"), providerRemember: $("provider-remember"),
    providerSettingsMessage: $("provider-settings-message"), providerSettingsCancel: $("provider-settings-cancel"),
    providerConsentDialog: $("provider-consent-dialog"), providerPayloadPreview: $("provider-payload-preview"),
    providerConsentCancel: $("provider-consent-cancel"), providerConsentConfirm: $("provider-consent-confirm"),
  };
  let cueNodes = [];
  let renderedTranscript = null;
  let handlers;
  let reviewWasOpen = false;
  let reviewReturnFocus = null;

  elements.feedForm.addEventListener("submit", (event) => { event.preventDefault(); handlers.loadFeed(elements.feedUrl.value); });
  elements.demo.addEventListener("click", () => handlers.loadDemo());
  elements.play.addEventListener("click", () => handlers.togglePlayback());
  elements.back.addEventListener("click", () => handlers.skip(-15));
  elements.forward.addEventListener("click", () => handlers.skip(30));
  elements.seek.addEventListener("input", () => handlers.seekFraction(Number(elements.seek.value) / 1000));
  elements.speed.addEventListener("change", () => handlers.setRate(Number(elements.speed.value)));
  elements.mute.addEventListener("click", () => handlers.toggleMute());
  elements.volume.addEventListener("input", () => handlers.setVolume(Number(elements.volume.value)));
  elements.follow.addEventListener("click", () => handlers.resumeFollow());
  elements.transcriptSelect.addEventListener("change", () => handlers.selectTranscript(elements.transcriptSelect.value));
  elements.lookupClose.addEventListener("click", () => handlers.closeLookup());
  elements.capture.addEventListener("click", () => handlers.captureLookup(elements.lookupChoice.value));
  elements.reviewButton.addEventListener("click", () => handlers.openReview());
  elements.reviewClose.addEventListener("click", () => handlers.closeReview());
  elements.reviewReveal.addEventListener("click", () => handlers.revealReview());
  elements.reviewPlay.addEventListener("click", () => handlers.playReviewSnippet());
  elements.reviewNext.addEventListener("click", () => handlers.advanceReview());
  elements.reviewContext.addEventListener("change", () => handlers.selectReviewContext(elements.reviewContext.value));
  elements.reviewRatings.addEventListener("click", (event) => {
    const button = event.target.closest(".review-rating");
    if (button) handlers.rateReview(Number(button.dataset.rating));
  });
  elements.disambiguate.addEventListener("click", () => handlers.requestDisambiguation());
  elements.providerSettingsButton.addEventListener("click", () => handlers.showDisambiguationSettings());
  elements.providerSettingsCancel.addEventListener("click", () => handlers.cancelDisambiguation());
  elements.providerConsentCancel.addEventListener("click", () => handlers.cancelDisambiguation());
  elements.providerConsentConfirm.addEventListener("click", () => handlers.confirmDisambiguation());
  elements.providerSettingsForm.addEventListener("submit", (event) => {
    event.preventDefault();
    handlers.saveDisambiguationSettings({
      adapterId: "openai_chat_completions",
      endpointUrl: elements.providerEndpoint.value,
      model: elements.providerModel.value,
      credential: elements.providerCredential.value,
      rememberCredential: elements.providerRemember.checked,
    });
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !elements.reviewDialog.hidden) {
      event.preventDefault();
      handlers.closeReview();
    }
  });
  elements.transcript.addEventListener("click", (event) => {
    const token = event.target.closest(".transcript-token");
    if (!token) return;
    handlers.lookupToken({ cueId: token.dataset.cueId, tokenIndex: Number(token.dataset.tokenIndex) });
  });
  for (const event of ["wheel", "touchstart", "pointerdown"]) {
    elements.transcript.addEventListener(event, () => handlers.manualFollow(), { passive: true });
  }
  elements.transcript.addEventListener("keydown", (event) => {
    if (event.target.matches(".transcript-token") && ["ArrowLeft", "ArrowRight"].includes(event.key)) {
      const tokens = [...elements.transcript.querySelectorAll(".transcript-token")];
      const index = tokens.indexOf(event.target);
      const next = event.key === "ArrowRight" ? Math.min(tokens.length - 1, index + 1) : Math.max(0, index - 1);
      tokens.forEach((token, tokenIndex) => { token.tabIndex = tokenIndex === next ? 0 : -1; });
      tokens[next]?.focus();
      event.preventDefault();
      return;
    }
    if (["ArrowDown", "ArrowUp", "PageDown", "PageUp", "Home", "End", " "].includes(event.key)) handlers.manualFollow();
  });

  function renderEpisodes(state) {
    const selected = state.workspace.selectedEpisodeId;
    elements.list.replaceChildren(...state.workspace.episodes.map((episode) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "episode-row";
      button.setAttribute("aria-current", String(episode.identity.internalId === selected));
      const image = document.createElement("img");
      image.src = episode.artworkUrl || "./assets/demo/cover.png";
      image.alt = "";
      const copy = document.createElement("span");
      const title = document.createElement("strong");
      title.textContent = episode.title;
      const meta = document.createElement("span");
      meta.textContent = episode.transcriptResources.some((item) => item.format) ? "Transcript available" : "Audio only";
      copy.append(title, meta);
      button.append(image, copy);
      button.addEventListener("click", () => handlers.selectEpisode(episode.identity.internalId));
      return button;
    }));
    elements.count.textContent = String(state.workspace.episodes.length);
  }

  function renderTranscript(episodeOpen) {
    const transcript = episodeOpen?.transcript;
    if (transcript === renderedTranscript) return;
    renderedTranscript = transcript;
    cueNodes = [];
    if (!transcript) {
      elements.transcript.replaceChildren();
      return;
    }
    const fragment = document.createDocumentFragment();
    let hasTabStop = false;
    transcript.cues.forEach((cue, index) => {
      const row = document.createElement("div");
      row.className = "cue";
      row.id = `cue-${index}`;
      const seek = document.createElement("button");
      seek.type = "button";
      seek.className = "cue-seek";
      seek.textContent = formatTime(cue.startMs / 1000);
      seek.setAttribute("aria-label", `Seek to ${seek.textContent}`);
      seek.addEventListener("click", () => handlers.seekSeconds(cue.startMs / 1000));
      const text = document.createElement("span");
      text.className = "cue-copy";
      let cursor = 0;
      cue.tokens.forEach((token, tokenIndex) => {
        text.append(document.createTextNode(cue.text.slice(cursor, token.startUtf16)));
        const button = document.createElement("button");
        button.type = "button";
        button.className = "transcript-token";
        button.textContent = cue.text.slice(token.startUtf16, token.endUtf16);
        button.dataset.cueId = cue.id;
        button.dataset.tokenIndex = String(tokenIndex);
        button.tabIndex = hasTabStop ? -1 : 0;
        hasTabStop = true;
        text.append(button);
        cursor = token.endUtf16;
      });
      text.append(document.createTextNode(cue.text.slice(cursor)));
      row.append(seek, text);
      fragment.append(row);
      cueNodes.push(row);
    });
    elements.transcript.replaceChildren(fragment);
  }

  function definitionList(entry) {
    const list = document.createElement("ol");
    list.className = "lookup-definitions";
    for (const definition of entry.definitions) {
      const item = document.createElement("li");
      const part = document.createElement("span");
      part.textContent = definition.partOfSpeech;
      item.append(part, document.createTextNode(definition.text));
      list.append(item);
    }
    return list;
  }

  function renderLookup(lookup) {
    const open = lookup.status !== "closed";
    elements.inspector.hidden = !open;
    elements.providerSettingsDialog.hidden = !open || lookup.ai.status !== "settings";
    elements.providerConsentDialog.hidden = !open || lookup.ai.status !== "consent";
    if (!open) return;
    const result = lookup.result;
    elements.lookupSurface.textContent = lookup.prepared?.surface ?? "Lookup";
    elements.lookupPhonetic.textContent = "";
    elements.lookupChoice.hidden = true;
    elements.lookupChoiceLabel.hidden = true;
    elements.capture.hidden = true;
    elements.capture.disabled = false;
    elements.disambiguationActions.hidden = true;
    elements.disambiguationResult.hidden = true;
    elements.providerSettingsMessage.textContent = lookup.ai.status === "settings" ? lookup.ai.message : "";
    elements.providerPayloadPreview.textContent = lookup.ai.prepared ? JSON.stringify(lookup.ai.prepared.request, null, 2) : "";
    elements.lookupBody.replaceChildren();
    if (lookup.status === "pending") elements.lookupBody.textContent = "Looking up locally…";
    else if (lookup.status === "capturing") elements.lookupBody.textContent = "Saving capture…";
    else if (lookup.status === "failed") elements.lookupBody.textContent = lookup.message;
    else if (lookup.status === "unknown") elements.lookupBody.textContent = "No offline lexicon entry found.";
    else if (["created_card", "added_encounter", "already_captured"].includes(lookup.status)) {
      elements.lookupBody.textContent = {
        created_card: "Saved as a new learning card.",
        added_encounter: "Saved this encounter to the existing card.",
        already_captured: "This encounter is already captured.",
      }[lookup.status];
    } else if (result?.status === "found") {
      elements.lookupSurface.textContent = result.entry.lemma;
      elements.lookupPhonetic.textContent = result.entry.phonetic;
      elements.lookupBody.append(definitionList(result.entry));
      elements.capture.hidden = false;
      elements.disambiguationActions.hidden = false;
    } else if (result?.status === "ambiguous") {
      elements.lookupBody.textContent = "Choose the matching lemma.";
      elements.lookupChoice.replaceChildren(...result.entries.map((entry) => {
        const option = document.createElement("option");
        option.value = entry.lemma;
        option.textContent = `${entry.lemma} · ${entry.definitions[0]?.partOfSpeech ?? "entry"}`;
        return option;
      }));
      elements.lookupChoice.hidden = false;
      elements.lookupChoiceLabel.hidden = false;
      elements.capture.hidden = false;
      elements.disambiguationActions.hidden = false;
    }
    if (["requesting", "ready", "failed"].includes(lookup.ai.status)) {
      elements.disambiguationResult.hidden = false;
      if (lookup.ai.status === "requesting") elements.disambiguationMessage.textContent = "Requesting contextual explanation…";
      else if (lookup.ai.status === "failed") elements.disambiguationMessage.textContent = lookup.ai.message;
      else elements.disambiguationMessage.textContent = lookup.ai.response.explanation;
    }
  }

  function renderReview(review) {
    const open = review.phase !== "closed";
    elements.reviewDialog.hidden = !open;
    elements.reviewDueCount.textContent = String(review.dueCount);
    elements.reviewButton.disabled = open;
    if (open && !reviewWasOpen) {
      reviewReturnFocus = document.activeElement;
      queueMicrotask(() => elements.reviewClose.focus());
    } else if (!open && reviewWasOpen) {
      reviewReturnFocus?.focus?.();
      reviewReturnFocus = null;
    }
    reviewWasOpen = open;
    if (!open) return;

    const card = review.cards[review.index];
    const context = card?.contexts.find((item) => item.contextId === review.selectedContextId) ?? card?.contexts[0];
    const activeCard = Boolean(card) && !["open", "complete", "exit"].includes(review.phase);
    elements.reviewContent.hidden = !activeCard;
    elements.reviewProgress.textContent = card ? `${review.index + 1} of ${review.cards.length}` : "Due cards";
    elements.reviewStatus.textContent = review.message;
    if (review.phase === "open") elements.reviewStatus.textContent = "Loading due cards…";
    if (review.phase === "complete") elements.reviewStatus.textContent = review.message || "Review complete.";
    if (review.phase === "exit") elements.reviewStatus.textContent = "Returning to the episode…";
    if (!activeCard) return;

    elements.reviewContext.replaceChildren(...card.contexts.map((item) => {
      const option = document.createElement("option");
      option.value = item.contextId;
      option.textContent = item.sourceLabel || "Saved context";
      option.selected = item.contextId === review.selectedContextId;
      return option;
    }));
    const multipleContexts = card.contexts.length > 1;
    elements.reviewContext.hidden = !multipleContexts;
    elements.reviewContextLabel.hidden = !multipleContexts;
    elements.reviewSentence.textContent = context?.sentence ?? "Saved context unavailable.";
    elements.reviewPlay.disabled = !context?.audioSlice || review.audio.status === "playing";
    elements.reviewPlay.textContent = review.audio.status === "playing" ? "Playing…" : "Replay snippet";
    elements.reviewAudioStatus.textContent = review.audio.message;

    const revealed = review.answerStatus === "ready";
    elements.reviewReveal.hidden = revealed || review.phase === "rated";
    elements.reviewReveal.disabled = review.phase === "revealing";
    elements.reviewReveal.textContent = review.phase === "revealing" ? "Revealing…" : "Reveal answer";
    elements.reviewAnswer.hidden = !revealed;
    elements.reviewRatings.hidden = !revealed || review.phase === "rated";
    elements.reviewRatings.querySelectorAll("button").forEach((button) => { button.disabled = review.saving; });
    elements.reviewNext.hidden = review.phase !== "rated";
    if (revealed) {
      elements.reviewLemma.textContent = review.answer.lemma || review.answer.term;
      elements.reviewPhonetic.textContent = review.answer.phonetic;
      elements.reviewDefinition.textContent = review.answer.definition;
    } else {
      elements.reviewLemma.textContent = "";
      elements.reviewPhonetic.textContent = "";
      elements.reviewDefinition.textContent = "";
    }
  }

  return {
    elements,
    bind(value) { handlers = value; },
    render(state) {
      renderEpisodes(state);
      const open = state.transcript.episode;
      const episode = open?.episode;
      elements.empty.hidden = Boolean(episode);
      elements.workspace.hidden = !episode;
      if (episode) {
        const feed = state.workspace.feeds.find((item) => item.sourceUrl === episode.identity.feedUrl);
        const artwork = episode.artworkUrl || feed?.artworkUrl || "./assets/demo/cover.png";
        elements.feedTitle.textContent = feed?.title ?? "Podcast";
        elements.title.textContent = episode.title;
        elements.playerTitle.textContent = episode.title;
        elements.playerFeed.textContent = feed?.title ?? "Podcast";
        elements.artwork.src = artwork;
        elements.playerArtwork.src = artwork;
        elements.transcriptSelect.replaceChildren(...episode.transcriptResources.map((resource, index) => {
          const option = document.createElement("option");
          option.value = resource.url;
          option.textContent = resourceLabel(resource, index);
          option.disabled = !resource.format;
          option.selected = resource.url === open.selectedTranscriptUrl;
          return option;
        }));
        elements.transcriptSelect.disabled = !episode.transcriptResources.some((resource) => resource.format);
        renderTranscript(open);
      }
      const statusCopy = {
        loading: "Loading transcript…", none: "This episode has no transcript. Audio playback is available.",
        unsupported_only: "Available transcripts use an unsupported format. Audio playback is available.",
        choice_required: "Choose a transcript to begin.", malformed: state.transcript.message || "The transcript is malformed.",
        empty: "This transcript contains no cues.", unavailable: state.transcript.message || "The transcript is unavailable.",
        offline: "This transcript is not cached for offline use.", ready: "", cached: "",
      };
      elements.transcriptStatus.textContent = statusCopy[state.transcript.status] ?? "";
      elements.follow.textContent = state.follow === "following" ? "Following" : "Return to current cue";
      elements.follow.dataset.follow = state.follow;
      elements.play.textContent = state.media.status === "playing" ? "Pause" : "Play";
      elements.play.setAttribute("aria-label", state.media.status === "playing" ? "Pause" : "Play");
      elements.elapsed.textContent = formatTime(state.media.currentTime);
      elements.remaining.textContent = `-${formatTime(Math.max(0, state.media.duration - state.media.currentTime))}`;
      elements.seek.value = state.media.duration ? String(Math.round(state.media.currentTime / state.media.duration * 1000)) : "0";
      elements.seek.disabled = !state.media.duration;
      elements.mute.textContent = state.media.muted ? "Unmute" : "Mute";
      elements.volume.value = String(state.media.volume);
      elements.network.textContent = state.capabilities.online ? "Local workspace" : "Offline · cached content only";
      renderLookup(state.lookup);
      renderReview(state.review);
    },
    updateSync(sync, follow) {
      const active = new Set(sync.activeCueIndices);
      cueNodes.forEach((node, index) => {
        node.classList.toggle("is-active", active.has(index));
        node.classList.toggle("is-anchor", sync.anchorCueIndex === index);
        if (sync.anchorCueIndex === index) node.setAttribute("aria-current", "true");
        else node.removeAttribute("aria-current");
      });
      if (follow === "following") {
        const index = sync.anchorCueIndex ?? sync.precedingCueIndex;
        cueNodes[index]?.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
      }
    },
  };
}
