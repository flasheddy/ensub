import { mapDatabaseRecord } from "./vocabulary-model.js";

const HISTORY_COLUMNS =
  "id,target_phrase,target_sentence,surrounding_context,lemma,part_of_speech,definition,nuance,confidence,created_at";

function throwIfError(error) {
  if (error) throw error;
}

export function createSupabaseClient() {
  const config = window.ENSUB_SUPABASE ?? {};
  if (!config.url || !config.publishableKey) {
    throw new Error("Supabase browser configuration is missing.");
  }
  if (typeof window.supabase?.createClient !== "function") {
    throw new Error("The Supabase client could not be loaded.");
  }
  return window.supabase.createClient(config.url, config.publishableKey, {
    auth: { persistSession: true, autoRefreshToken: true, detectSessionInUrl: false },
  });
}

export async function ensureAnonymousSession(client) {
  const { data: current, error: currentError } = await client.auth.getSession();
  throwIfError(currentError);
  if (current.session) return current.session;

  const { data: created, error: createError } = await client.auth.signInAnonymously();
  throwIfError(createError);
  if (!created.session) throw new Error("Anonymous authentication did not return a session.");
  return created.session;
}

export async function loadHistoryPage(client, offset = 0, pageSize = 20) {
  const { data, error } = await client
    .from("vocabulary_records")
    .select(HISTORY_COLUMNS)
    .order("created_at", { ascending: false })
    .order("id", { ascending: false })
    .range(offset, offset + pageSize);
  throwIfError(error);
  const rows = data ?? [];
  return {
    records: rows.slice(0, pageSize).map(mapDatabaseRecord),
    hasMore: rows.length > pageSize,
  };
}

export async function analyzeVocabulary(client, input, requestId) {
  const { data, error } = await client.functions.invoke("analyze-vocabulary", {
    body: { requestId, ...input },
  });
  throwIfError(error);
  if (!data?.record) throw new Error("Analysis returned no saved record.");
  return data.record;
}

export function toUserMessage(error) {
  const status = error?.status ?? error?.context?.status;
  if (status === 401) return "Your private session expired. Reconnect and try again.";
  if (status === 429) return "Analysis is busy right now. Wait a moment and try again.";
  if (status === 504) return "The analysis took too long. Try again in a moment.";
  if (status === 502) return "The language service returned an unreadable result. Try again.";
  return "We could not analyze and save this capture. Please try again.";
}
