use ensub_llm::{resolve_contextual_meaning, DisambiguationRequest, DisambiguationResult};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const SYSTEM_PROMPT: &str = "You are a precise lexicographer and ESL educator. Your task is to analyze a target word or phrase within a specific sentence and surrounding context. Determine the exact contextual meaning, part of speech, and lemma. If the context is too ambiguous to determine the exact meaning with high certainty, set 'confidence' to 'low'. Output MUST be a valid JSON object matching the requested schema exactly, with no additional markdown, commentary, or wrapper text.";

struct CapturedRequest {
    request_line: String,
    headers: HashMap<String, String>,
    body: String,
}

fn spawn_server(
    status: &str,
    response_body: impl Into<String>,
) -> (String, Receiver<CapturedRequest>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server must bind");
    let address = listener
        .local_addr()
        .expect("test server address must resolve");
    let (sender, receiver) = mpsc::channel();
    let status = status.to_string();
    let response_body = response_body.into();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("test server must accept a request");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("test server must set a read timeout");

        let request = read_request(&mut stream);
        sender
            .send(request)
            .expect("captured request must reach the test");

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("test server must write its response");
    });

    (format!("http://{address}"), receiver, handle)
}

fn read_request(stream: &mut impl Read) -> CapturedRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let count = stream
            .read(&mut buffer)
            .expect("request bytes must be readable");
        assert!(count > 0, "client closed before sending a complete request");
        bytes.extend_from_slice(&buffer[..count]);

        let Some(header_end) = find_bytes(&bytes, b"\r\n\r\n") else {
            continue;
        };
        let header_text =
            String::from_utf8(bytes[..header_end].to_vec()).expect("request headers must be UTF-8");
        let content_length = header_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("content length must parse")
                })
            })
            .unwrap_or(0);
        let body_start = header_end + 4;
        if bytes.len() >= body_start + content_length {
            break;
        }
    }

    let header_end = find_bytes(&bytes, b"\r\n\r\n").expect("complete request must have headers");
    let header_text =
        String::from_utf8(bytes[..header_end].to_vec()).expect("request headers must be UTF-8");
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .expect("request line must be present")
        .to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let body =
        String::from_utf8(bytes[header_end + 4..].to_vec()).expect("request body must be UTF-8");

    CapturedRequest {
        request_line,
        headers,
        body,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn completion_response(content: &str) -> String {
    json!({
        "choices": [{
            "message": {
                "content": content,
            },
        }],
    })
    .to_string()
}

fn request(context: Option<&str>) -> DisambiguationRequest {
    DisambiguationRequest {
        target_phrase: "ran out".to_string(),
        target_sentence: "We ran out of time before the train arrived.".to_string(),
        surrounding_context: context.map(str::to_string),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sends_exact_contract_and_parses_first_choice() {
    let response = completion_response(
        r#"{"lemma":"run out","part_of_speech":"phrasal verb","definition":"Use all of something so none remains.","nuance":"Implies depletion before a task or need was complete.","confidence":"high"}"#,
    );
    let (base_url, receiver, handle) = spawn_server("200 OK", response);
    let client = reqwest::Client::new();

    let result = resolve_contextual_meaning(
        &client,
        &format!("{base_url}/v1/"),
        "secret-key",
        "provider-model",
        request(None),
    )
    .await
    .expect("valid completion must resolve");

    assert_result(&result);
    let captured = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("request must be captured");
    handle.join().expect("test server must finish");
    assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(
        captured.headers.get("authorization").map(String::as_str),
        Some("Bearer secret-key")
    );

    let body: Value = serde_json::from_str(&captured.body).expect("request body must be JSON");
    assert_eq!(body["model"], json!("provider-model"));
    assert_eq!(body["response_format"], json!({ "type": "json_object" }));
    assert_eq!(
        body["messages"][0],
        json!({
            "role": "system",
            "content": SYSTEM_PROMPT,
        })
    );
    assert_eq!(
        body["messages"][1],
        json!({
            "role": "user",
            "content": expected_user_prompt(""),
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn preserves_context_without_requiring_a_trailing_base_url_slash() {
    let response = completion_response(
        r#"{"lemma":"run out","part_of_speech":"phrasal verb","definition":"Use all of something so none remains.","nuance":"Implies depletion before a task or need was complete.","confidence":"high"}"#,
    );
    let (base_url, receiver, handle) = spawn_server("200 OK", response);

    resolve_contextual_meaning(
        &reqwest::Client::new(),
        &base_url,
        "secret-key",
        "provider-model",
        request(Some("The group was rushing to meet a deadline.")),
    )
    .await
    .expect("valid completion must resolve");

    let captured = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("request must be captured");
    handle.join().expect("test server must finish");
    assert_eq!(captured.request_line, "POST /chat/completions HTTP/1.1");
    let body: Value = serde_json::from_str(&captured.body).expect("request body must be JSON");
    assert_eq!(
        body["messages"][1]["content"],
        json!(expected_user_prompt(
            "The group was rushing to meet a deadline."
        ))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn rejects_non_success_statuses() {
    let (base_url, receiver, handle) = spawn_server(
        "400 Bad Request",
        r#"{"error":{"message":"response format unsupported"}}"#,
    );

    let error = resolve_contextual_meaning(
        &reqwest::Client::new(),
        &base_url,
        "secret-key",
        "provider-model",
        request(None),
    )
    .await
    .expect_err("non-success status must fail");

    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("request must be captured");
    handle.join().expect("test server must finish");
    assert!(error.to_string().contains("400"));
}

#[tokio::test(flavor = "current_thread")]
async fn rejects_an_empty_choices_array() {
    let (base_url, receiver, handle) = spawn_server("200 OK", r#"{"choices":[]}"#);

    let error = resolve_contextual_meaning(
        &reqwest::Client::new(),
        &base_url,
        "secret-key",
        "provider-model",
        request(None),
    )
    .await
    .expect_err("empty choices must fail");

    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("request must be captured");
    handle.join().expect("test server must finish");
    assert!(error.to_string().contains("no choices"));
}

#[tokio::test(flavor = "current_thread")]
async fn rejects_missing_choice_content() {
    let (base_url, receiver, handle) = spawn_server("200 OK", r#"{"choices":[{"message":{}}]}"#);

    let result = resolve_contextual_meaning(
        &reqwest::Client::new(),
        &base_url,
        "secret-key",
        "provider-model",
        request(None),
    )
    .await;

    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("request must be captured");
    handle.join().expect("test server must finish");
    assert!(result.is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn rejects_malformed_choice_content() {
    let response = completion_response("```json\n{}\n```");
    let (base_url, receiver, handle) = spawn_server("200 OK", response);

    let result = resolve_contextual_meaning(
        &reqwest::Client::new(),
        &base_url,
        "secret-key",
        "provider-model",
        request(None),
    )
    .await;

    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("request must be captured");
    handle.join().expect("test server must finish");
    assert!(result.is_err());
}

fn assert_result(result: &DisambiguationResult) {
    assert_eq!(result.lemma, "run out");
    assert_eq!(result.part_of_speech, "phrasal verb");
    assert_eq!(result.definition, "Use all of something so none remains.");
    assert_eq!(
        result.nuance,
        "Implies depletion before a task or need was complete."
    );
    assert_eq!(result.confidence, "high");
}

fn expected_user_prompt(context: &str) -> String {
    format!(
        r#"Target: "ran out"
Sentence: "We ran out of time before the train arrived."
Context: "{context}"

Respond strictly with this JSON format:
{{
  "lemma": "dictionary base form",
  "part_of_speech": "noun | verb | phrasal verb | idiom | adjective | adverb | other",
  "definition": "Clear, concise definition strictly matching this context (max 20 words)",
  "nuance": "Brief note on tone, connotation, or why this sense applies (max 25 words)",
  "confidence": "high" | "low"
}}"#
    )
}
