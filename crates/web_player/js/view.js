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
  for (const event of ["wheel", "touchstart", "pointerdown"]) {
    elements.transcript.addEventListener(event, () => handlers.manualFollow(), { passive: true });
  }
  elements.transcript.addEventListener("keydown", (event) => {
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
      text.textContent = cue.text;
      row.append(seek, text);
      fragment.append(row);
      cueNodes.push(row);
    });
    elements.transcript.replaceChildren(fragment);
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
