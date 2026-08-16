import {
  analyzeVocabulary,
  createSupabaseClient,
  ensureAnonymousSession,
  loadHistoryPage,
  toUserMessage,
} from "./supabase-api.js";
import { mergeHistoryRecord, validateCaptureInput } from "./vocabulary-model.js";
import {
  clearNotice,
  readCaptureForm,
  renderHistory,
  renderResult,
  setBootstrapError,
  setFieldErrors,
  setHistoryLoading,
  setReady,
  setSubmitting,
  showNotice,
} from "./vocabulary-view.js";

const PAGE_SIZE = 20;

const state = {
  client: null,
  ready: false,
  records: [],
  offset: 0,
  hasMore: false,
  loadingHistory: false,
};

async function loadFirstPage() {
  setHistoryLoading(true);
  const page = await loadHistoryPage(state.client, 0, PAGE_SIZE);
  state.records = page.records;
  state.offset = page.records.length;
  state.hasMore = page.hasMore;
  renderHistory(state.records, state.hasMore);
}

async function bootstrap() {
  state.ready = false;
  setReady(false);
  setBootstrapError(null);
  try {
    state.client ??= createSupabaseClient();
    await ensureAnonymousSession(state.client);
    await loadFirstPage();
    state.ready = true;
    setReady(true);
  } catch {
    setHistoryLoading(false, "History is unavailable until the private session reconnects.");
    setBootstrapError("Ensub could not start a private session. Check your connection and try again.");
  }
}

async function submitCapture(event) {
  event.preventDefault();
  const form = event.currentTarget;
  clearNotice();
  const validation = validateCaptureInput(readCaptureForm());
  setFieldErrors(validation.errors);
  if (Object.keys(validation.errors).length > 0) {
    showNotice("Check the highlighted fields before analyzing.", "error");
    document.querySelector("[aria-invalid='true']")?.focus();
    return;
  }
  if (!state.ready) {
    showNotice("Reconnect the private session before analyzing.", "error");
    return;
  }

  setSubmitting(true);
  try {
    const record = await analyzeVocabulary(state.client, validation.value, crypto.randomUUID());
    const existed = state.records.some((item) => item.id === record.id);
    state.records = mergeHistoryRecord(state.records, record);
    if (!existed) state.offset += 1;
    renderResult(record);
    renderHistory(state.records, state.hasMore);
    form.reset();
    setFieldErrors({});
    showNotice("Analysis saved to your private history.", "success");
    document.querySelector("#target-phrase")?.focus();
  } catch (error) {
    showNotice(toUserMessage(error), "error");
  } finally {
    setSubmitting(false);
  }
}

async function loadMoreHistory() {
  if (state.loadingHistory || !state.hasMore) return;
  state.loadingHistory = true;
  setHistoryLoading(true, null, { preserve: true });
  try {
    const page = await loadHistoryPage(state.client, state.offset, PAGE_SIZE);
    state.records = [...state.records, ...page.records];
    state.offset += page.records.length;
    state.hasMore = page.hasMore;
    renderHistory(state.records, state.hasMore);
  } catch {
    showNotice("More captures could not be loaded. Try again.", "error");
    renderHistory(state.records, state.hasMore);
  } finally {
    state.loadingHistory = false;
  }
}

document.querySelector("#capture-form")?.addEventListener("submit", submitCapture);
document.querySelector("#retry-bootstrap")?.addEventListener("click", bootstrap);
document.querySelector("#load-more")?.addEventListener("click", loadMoreHistory);

void bootstrap();
