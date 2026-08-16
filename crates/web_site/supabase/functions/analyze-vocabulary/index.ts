import "jsr:@supabase/functions-js/edge-runtime.d.ts";
import { createClient } from "npm:@supabase/supabase-js@2.57.4";
import {
  buildCompletionRequest,
  parseModelContent,
  toApiRecord,
  validateAnalyzeRequest,
} from "./contract.mjs";

const RECORD_COLUMNS =
  "id,target_phrase,target_sentence,surrounding_context,lemma,part_of_speech,definition,nuance,confidence,created_at";
const LOCAL_ORIGINS = ["http://127.0.0.1:4173", "http://localhost:4173"];

class HttpError extends Error {
  status: number;
  code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

function allowedOrigins(): Set<string> {
  const configured = (Deno.env.get("ALLOWED_ORIGINS") ?? "")
    .split(",")
    .map((origin) => origin.trim())
    .filter(Boolean);
  return new Set([...LOCAL_ORIGINS, ...configured]);
}

function corsHeaders(origin: string | null): HeadersInit {
  const headers: Record<string, string> = {
    "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
    "Access-Control-Allow-Methods": "POST, OPTIONS",
    "Content-Type": "application/json",
    Vary: "Origin",
  };
  if (origin && allowedOrigins().has(origin)) headers["Access-Control-Allow-Origin"] = origin;
  return headers;
}

function jsonResponse(body: unknown, status: number, origin: string | null): Response {
  return new Response(JSON.stringify(body), { status, headers: corsHeaders(origin) });
}

function providerConfiguration() {
  const baseUrl = Deno.env.get("LLM_BASE_URL")?.trim().replace(/\/+$/, "");
  const apiKey = Deno.env.get("LLM_API_KEY")?.trim();
  const model = Deno.env.get("LLM_MODEL")?.trim();
  if (!baseUrl || !apiKey || !model) {
    throw new HttpError(500, "configuration_error", "The analysis service is not configured.");
  }
  return { baseUrl, apiKey, model };
}

async function completeAnalysis(input: ReturnType<typeof validateAnalyzeRequest>) {
  const config = providerConfiguration();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 25_000);

  let response: Response;
  try {
    response = await fetch(`${config.baseUrl}/chat/completions`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${config.apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(buildCompletionRequest(input, config.model)),
      signal: controller.signal,
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") {
      throw new HttpError(504, "provider_timeout", "The analysis service timed out.");
    }
    throw new HttpError(502, "provider_unavailable", "The analysis service could not be reached.");
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    if (response.status === 429) {
      throw new HttpError(429, "provider_rate_limited", "The analysis service is busy.");
    }
    throw new HttpError(502, "provider_error", "The analysis service rejected the request.");
  }

  let completion: unknown;
  try {
    completion = await response.json();
  } catch {
    throw new HttpError(502, "invalid_provider_response", "The analysis response was not JSON.");
  }
  const content = (completion as { choices?: Array<{ message?: { content?: unknown } }> })
    ?.choices?.[0]?.message?.content;
  if (typeof content !== "string") {
    throw new HttpError(502, "invalid_provider_response", "The analysis response had no content.");
  }
  try {
    return parseModelContent(content);
  } catch {
    throw new HttpError(502, "invalid_provider_response", "The analysis response was invalid.");
  }
}

Deno.serve(async (request: Request) => {
  const origin = request.headers.get("Origin");
  if (origin && !allowedOrigins().has(origin)) {
    return jsonResponse({ error: { code: "origin_not_allowed", message: "Origin not allowed." } }, 403, origin);
  }
  if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: corsHeaders(origin) });
  if (request.method !== "POST") {
    return jsonResponse({ error: { code: "method_not_allowed", message: "Use POST." } }, 405, origin);
  }

  try {
    const authorization = request.headers.get("Authorization");
    if (!authorization?.startsWith("Bearer ")) {
      throw new HttpError(401, "authentication_required", "A private session is required.");
    }
    const supabaseUrl = Deno.env.get("SUPABASE_URL");
    const publicKey = Deno.env.get("SUPABASE_ANON_KEY");
    if (!supabaseUrl || !publicKey) {
      throw new HttpError(500, "configuration_error", "The data service is not configured.");
    }
    const supabase = createClient(supabaseUrl, publicKey, {
      global: { headers: { Authorization: authorization } },
      auth: { persistSession: false, autoRefreshToken: false },
    });
    const { data: authData, error: authError } = await supabase.auth.getUser();
    if (authError || !authData.user) {
      throw new HttpError(401, "authentication_required", "A private session is required.");
    }

    let payload: unknown;
    try {
      payload = await request.json();
    } catch {
      throw new HttpError(400, "invalid_request", "Request body must be valid JSON.");
    }
    let input: ReturnType<typeof validateAnalyzeRequest>;
    try {
      input = validateAnalyzeRequest(payload);
    } catch (error) {
      throw new HttpError(400, "invalid_request", error instanceof Error ? error.message : "Invalid request.");
    }

    const { data: existing, error: existingError } = await supabase
      .from("vocabulary_records")
      .select(RECORD_COLUMNS)
      .eq("id", input.requestId)
      .maybeSingle();
    if (existingError) throw new HttpError(500, "database_error", "Saved records could not be checked.");
    if (existing) return jsonResponse({ record: toApiRecord(existing) }, 200, origin);

    const analysis = await completeAnalysis(input);
    const { data: inserted, error: insertError } = await supabase
      .from("vocabulary_records")
      .insert({
        id: input.requestId,
        user_id: authData.user.id,
        target_phrase: input.targetPhrase,
        target_sentence: input.targetSentence,
        surrounding_context: input.surroundingContext,
        lemma: analysis.lemma,
        part_of_speech: analysis.partOfSpeech,
        definition: analysis.definition,
        nuance: analysis.nuance,
        confidence: analysis.confidence,
      })
      .select(RECORD_COLUMNS)
      .single();
    if (insertError || !inserted) {
      throw new HttpError(500, "database_error", "The analysis could not be saved.");
    }
    return jsonResponse({ record: toApiRecord(inserted) }, 200, origin);
  } catch (error) {
    const failure = error instanceof HttpError
      ? error
      : new HttpError(500, "internal_error", "The request could not be completed.");
    console.error("analyze-vocabulary failed", { code: failure.code, status: failure.status });
    return jsonResponse({ error: { code: failure.code, message: failure.message } }, failure.status, origin);
  }
});
