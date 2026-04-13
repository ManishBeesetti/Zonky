use serde_json::Value;

/// Format a StreamChunk as an SSE event line
pub fn format_sse_event(data: &Value) -> String {
    format!("data: {}\n\n", serde_json::to_string(data).unwrap_or_default())
}

/// Format the SSE done marker
pub fn format_sse_done() -> String {
    "data: [DONE]\n\n".to_string()
}
