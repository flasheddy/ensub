use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::io;

const SYSTEM_PROMPT: &str = "You are a precise lexicographer and ESL educator. Your task is to analyze a target word or phrase within a specific sentence and surrounding context. Determine the exact contextual meaning, part of speech, and lemma. If the context is too ambiguous to determine the exact meaning with high certainty, set 'confidence' to 'low'. Output MUST be a valid JSON object matching the requested schema exactly, with no additional markdown, commentary, or wrapper text.";

/// Context needed to resolve the intended sense of a word or phrase.
pub struct DisambiguationRequest {
    pub target_phrase: String,
    pub target_sentence: String,
    pub surrounding_context: Option<String>,
}

/// Context-specific lexical information returned by the language model.
#[derive(Serialize, Deserialize, Debug)]
pub struct DisambiguationResult {
    pub lemma: String,
    pub part_of_speech: String,
    pub definition: String,
    pub nuance: String,
    pub confidence: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// Resolves the contextual meaning through an OpenAI-compatible Chat Completions endpoint.
pub async fn resolve_contextual_meaning(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    req: DisambiguationRequest,
) -> Result<DisambiguationResult, Box<dyn Error>> {
    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let context = req.surrounding_context.unwrap_or_default();
    let user_prompt = format!(
        r#"Target: "{}"
Sentence: "{}"
Context: "{}"

Respond strictly with this JSON format:
{{
  "lemma": "dictionary base form",
  "part_of_speech": "noun | verb | phrasal verb | idiom | adjective | adverb | other",
  "definition": "Clear, concise definition strictly matching this context (max 20 words)",
  "nuance": "Brief note on tone, connotation, or why this sense applies (max 25 words)",
  "confidence": "high" | "low"
}}"#,
        req.target_phrase, req.target_sentence, context
    );
    let request_body = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": SYSTEM_PROMPT,
            },
            {
                "role": "user",
                "content": user_prompt,
            },
        ],
        "response_format": {
            "type": "json_object",
        },
    });

    let completion = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await?
        .error_for_status()?
        .json::<ChatCompletionResponse>()
        .await?;
    let content = completion
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "response contained no choices"))?
        .message
        .content;

    Ok(serde_json::from_str::<DisambiguationResult>(&content)?)
}
