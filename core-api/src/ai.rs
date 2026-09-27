use crate::error::ApiError;
use crate::{ApiResult, AppState};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub message: String,
}

#[derive(Debug, Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

/// POST /v1/ai/chat
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> ApiResult<Json<ChatResponse>> {
    let message = req.message.trim();

    if message.is_empty() {
        return Err(ApiError::Validation(
            "message must not be empty".to_string(),
        ));
    }

    let request = OllamaRequest {
        model: &state.ollama_model,
        messages: vec![OllamaMessage {
            role: "user",
            content: message,
        }],
        stream: false,
    };

    let response = state
        .http_client
        .post(format!("{}/api/chat", state.ollama_url))
        .timeout(Duration::from_secs(60))
        .json(&request)
        .send()
        .await
        .map_err(|e| ApiError::Unavailable(format!("Ollama request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        return Err(ApiError::Unavailable(format!(
            "Ollama returned HTTP {status}: {body}"
        )));
    }

    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| ApiError::Unavailable(format!("Invalid Ollama response: {e}")))?;

    Ok(Json(ChatResponse {
        message: ollama_response.message.content,
    }))
}
