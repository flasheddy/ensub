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
  };
  let cueNodes = [];
  let renderedTranscript = null;
  let handlers;

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
    if (!open) return;
    const result = lookup.result;
    elements.lookupSurface.textContent = lookup.prepared?.surface ?? "Lookup";
    elements.lookupPhonetic.textContent = "";
    elements.lookupChoice.hidden = true;
    elements.lookupChoiceLabel.hidden = true;
    elements.capture.hidden = true;
    elements.capture.disabled = false;
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
