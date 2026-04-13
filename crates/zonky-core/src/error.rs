use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZonkyError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model already loaded: {0}")]
    ModelAlreadyLoaded(String),

    #[error("Inference error: {0}")]
    InferenceError(String),

    #[error("Backend error: {0}")]
    BackendError(String),

    #[error("HuggingFace Hub error: {0}")]
    HubError(String),

    #[error("GPU error: {0}")]
    GpuError(String),

    #[error("Insufficient VRAM: need {needed} bytes, available {available} bytes")]
    InsufficientVram { needed: u64, available: u64 },

    #[error("Unsupported model architecture: {0}")]
    UnsupportedArchitecture(String),

    #[error("Invalid model format: {0}")]
    InvalidModelFormat(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Download error: {0}")]
    DownloadError(String),

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Candle error: {0}")]
    CandleError(#[from] candle_core::Error),
}

pub type Result<T> = std::result::Result<T, ZonkyError>;
