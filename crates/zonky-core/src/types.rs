use serde::{Deserialize, Serialize};

/// A chat message with role and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Request for text generation (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_top_p")]
    pub top_p: f64,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub stream: bool,
    #[serde(default = "default_repetition_penalty")]
    pub repetition_penalty: f64,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub frequency_penalty: f64,
    #[serde(default)]
    pub presence_penalty: f64,
    #[serde(default)]
    pub seed: Option<u64>,
}

fn default_temperature() -> f64 {
    0.7
}
fn default_top_p() -> f64 {
    1.0
}
fn default_max_tokens() -> u32 {
    2048
}
fn default_repetition_penalty() -> f64 {
    1.1
}

/// Completion request (legacy /v1/completions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_top_p")]
    pub top_p: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
}

/// Full generation response (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk (SSE data payload)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Information about a loaded or available model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_usage: Option<u64>,
    pub loaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Which inference backends can load this model
    #[serde(default)]
    pub compatible_backends: Vec<String>,
}

/// Model list response (OpenAI-compatible GET /v1/models)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

/// Backend choice for model loading
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackendChoice {
    Candle,
    LlamaCpp,
    Auto,
}

impl Default for BackendChoice {
    fn default() -> Self {
        Self::Auto
    }
}

/// Device choice for inference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceChoice {
    Auto,
    Cpu,
    Cuda(usize),
    Metal,
}

impl Default for DeviceChoice {
    fn default() -> Self {
        Self::Auto
    }
}

/// Download progress information
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed_bytes_per_sec: u64,
}

/// HuggingFace model search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelInfo {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub downloads: Option<u64>,
    #[serde(default)]
    pub likes: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pipeline_tag: Option<String>,
    #[serde(default)]
    pub library_name: Option<String>,
    #[serde(default, rename = "lastModified")]
    pub last_modified: Option<String>,
}

/// Sibling file entry from HF API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSibling {
    #[serde(rename = "rfilename")]
    pub filename: String,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Detailed model info from HF API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelDetail {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(default, rename = "id")]
    pub _id_dup: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pipeline_tag: Option<String>,
    #[serde(default)]
    pub library_name: Option<String>,
    #[serde(default)]
    pub siblings: Vec<HubSibling>,
    #[serde(default)]
    pub downloads: Option<u64>,
    #[serde(default)]
    pub likes: Option<u64>,
}

/// Local model registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModel {
    pub id: String,
    pub repo_id: String,
    pub filename: String,
    pub path: std::path::PathBuf,
    pub file_size: u64,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub quantization: Option<String>,
    #[serde(default)]
    pub architecture: Option<String>,
    pub downloaded_at: String,
}

/// System information for health/status endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub loaded_models: Vec<String>,
    pub gpu_devices: Vec<GpuInfo>,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub device_type: String,
    pub vram_total: Option<u64>,
    pub vram_free: Option<u64>,
}
