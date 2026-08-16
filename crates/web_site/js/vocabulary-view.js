import { formatConfidence } from "./vocabulary-model.js";

const fields = {
  targetPhrase: document.querySelector("#target-phrase"),
  targetSentence: document.querySelector("#target-sentence"),
  surroundingContext: document.querySelector("#surrounding-context"),
};

const errors = {
  targetPhrase: document.querySelector("#target-phrase-error"),
  targetSentence: document.querySelector("#target-sentence-error"),
  surroundingContext: document.querySelector("#surrounding-context-error"),
};

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function appendBadge(parent, label, value) {
  const badge = element("span", "result-badge", label);
  badge.append(element("strong", "", value));
  parent.append(badge);
}

function formatDate(value) {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "Saved recently";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function readCaptureForm() {
  return {
    targetPhrase: fields.targetPhrase?.value ?? "",
    targetSentence: fields.targetSentence?.value ?? "",
    surroundingContext: fields.surroundingContext?.value ?? "",
  };
}

export function setFieldErrors(fieldErrors) {
  for (const name of Object.keys(fields)) {
    const message = fieldErrors[name] ?? "";
    errors[name].textContent = message;
    fields[name].setAttribute("aria-invalid", message ? "true" : "false");
  }
}

export function setReady(ready) {
  const button = document.querySelector("#analyze-button");
  const session = document.querySelector("#session-state");
  button.disabled = !ready;
  session.textContent = ready ? "Private session ready" : "Connecting";
  session.className = ready
    ? "mt-1 shrink-0 text-xs font-semibold text-[#3f795d]"
    : "mt-1 shrink-0 text-xs font-semibold text-[#6b7973]";
}

export function setBootstrapError(message) {
  const alert = document.querySelector("#bootstrap-alert");
  const session = document.querySelector("#session-state");
  document.querySelector("#bootstrap-message").textContent = message ?? "";
  alert.classList.toggle("hidden", !message);
  alert.classList.toggle("flex", Boolean(message));
  if (message) {
    session.textContent = "Session unavailable";
    session.className = "mt-1 shrink-0 text-xs font-semibold text-[#a13f35]";
  }
}

export function setSubmitting(submitting) {
  const button = document.querySelector("#analyze-button");
  document.querySelector("#analyze-spinner").classList.toggle("hidden", !submitting);
  document.querySelector("#analyze-label").textContent = submitting ? "Analyzing & saving…" : "Analyze & Save";
  button.disabled = submitting;
  document.querySelector("#capture-form").setAttribute("aria-busy", String(submitting));
}

export function showNotice(message, type) {
  const notice = document.querySelector("#form-notice");
  notice.textContent = message;
  notice.className = type === "success"
    ? "mt-5 rounded-[5px] border border-[#acd0ba] bg-[#edf8f1] px-4 py-3 text-sm text-[#23573d]"
    : "mt-5 rounded-[5px] border border-[#dfb8b1] bg-[#fff3f0] px-4 py-3 text-sm text-[#7b3027]";
}

export function clearNotice() {
  const notice = document.querySelector("#form-notice");
  notice.textContent = "";
  notice.className = "mt-5 hidden rounded-[5px] border px-4 py-3 text-sm";
}

export function renderResult(record) {
  const panel = document.querySelector("#result-panel");
  panel.className = "block min-h-[355px] pt-6";
  panel.replaceChildren();

  const badges = element("div", "flex flex-wrap gap-2");
  appendBadge(badges, "Lemma", record.lemma);
  appendBadge(badges, "Part of speech", record.partOfSpeech);
  appendBadge(badges, "Confidence", formatConfidence(record.confidence));

  const phrase = element("p", "mt-7 font-serif text-4xl font-semibold leading-tight text-white", record.targetPhrase);
  const sentence = element("blockquote", "mt-4 border-l-2 border-[#d7f06b] pl-4 font-serif text-lg leading-8 text-[#e7f0eb]", record.targetSentence);

  const definitionLabel = element("p", "mt-7 text-[10px] font-bold uppercase text-[#9fb9ac]", "Definition in context");
  const definition = element("p", "mt-2 text-base leading-7 text-white", record.definition);
  const nuanceLabel = element("p", "mt-6 text-[10px] font-bold uppercase text-[#9fb9ac]", "Nuance");
  const nuance = element("p", "mt-2 text-sm leading-6 text-[#c8d8d0]", record.nuance);

  panel.append(badges, phrase, sentence, definitionLabel, definition, nuanceLabel, nuance);
}

function historyCard(record) {
  const article = element("article", "history-card");
  const meta = element("div", "history-meta");
  meta.append(
    element("h3", "history-phrase", record.targetPhrase),
    element("span", "history-time", formatDate(record.createdAt)),
  );

  const body = element("div", "history-body");
  body.append(element("p", "history-sentence", record.targetSentence));
  if (record.surroundingContext) {
    body.append(element("p", "history-context", record.surroundingContext));
  }

  const analysis = element("dl", "history-analysis");
  const entries = [
    ["Lemma", record.lemma],
    ["Part of speech", record.partOfSpeech],
    ["Definition", record.definition],
    ["Nuance", record.nuance],
    ["Confidence", formatConfidence(record.confidence)],
  ];
  for (const [label, value] of entries) {
    const group = element("div");
    group.append(element("dt", "", label), element("dd", "", value));
    analysis.append(group);
  }
  body.append(analysis);
  article.append(meta, body);
  return article;
}

export function renderHistory(records, hasMore) {
  const list = document.querySelector("#history-list");
  list.setAttribute("aria-busy", "false");
  list.replaceChildren();
  if (records.length === 0) {
    list.append(element("div", "history-state", "No captures yet. Your first saved analysis will appear here."));
  } else {
    list.append(...records.map(historyCard));
  }
  document.querySelector("#history-count").textContent = `${records.length} ${records.length === 1 ? "capture" : "captures"}`;
  const loadMore = document.querySelector("#load-more");
  loadMore.classList.toggle("hidden", !hasMore);
  loadMore.disabled = false;
  loadMore.textContent = "Load more";
}

export function setHistoryLoading(loading, message, options = {}) {
  const list = document.querySelector("#history-list");
  list.setAttribute("aria-busy", String(loading));
  if (options.preserve) {
    const loadMore = document.querySelector("#load-more");
    loadMore.disabled = loading;
    loadMore.textContent = loading ? "Loading…" : "Load more";
    return;
  }
  if (message) list.replaceChildren(element("div", "history-state", message));
  document.querySelector("#history-count").textContent = loading ? "Loading history" : "History unavailable";
}
