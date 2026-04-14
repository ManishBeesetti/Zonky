pub mod candle_backend;
#[cfg(feature = "llamacpp")]
pub mod llamacpp_backend;

use std::path::Path;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::gpu::GpuDevice;
use crate::types::{GenerationRequest, GenerationResponse, ModelInfo, StreamChunk};

/// Result of a lightweight compatibility probe
#[derive(Debug, Clone)]
pub struct BackendCompatibility {
    /// Whether this backend can load the model
    pub compatible: bool,
    /// Human-readable reason if incompatible
    pub reason: Option<String>,
}

/// Opaque handle to a loaded model within a backend
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelHandle {
    pub id: String,
    pub backend: String,
}

/// Trait for pluggable inference backends
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Backend name (e.g. "candle", "llamacpp")
    fn name(&self) -> &str;

    /// Load a model from the given path onto the specified device
    async fn load_model(
        &self,
        model_id: &str,
        path: &Path,
        device: &GpuDevice,
    ) -> Result<ModelHandle>;

    /// Generate a complete response (non-streaming)
    async fn generate(
        &self,
        handle: &ModelHandle,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse>;

    /// Generate a streaming response, sending chunks through the channel
    async fn generate_stream(
        &self,
        handle: &ModelHandle,
        request: &GenerationRequest,
        tx: mpsc::Sender<Result<StreamChunk>>,
    ) -> Result<()>;

    /// Unload a model and free its resources
    async fn unload_model(&self, handle: &ModelHandle) -> Result<()>;

    /// Get information about a loaded model
    fn model_info(&self, handle: &ModelHandle) -> Option<ModelInfo>;

    /// Estimate VRAM required for a model file (in bytes)
    fn estimate_vram(&self, path: &Path) -> Result<u64>;

    /// Quick probe to check if this backend can load the given model file.
    /// Must be lightweight — read headers/metadata only, do not load weights.
    fn can_load(&self, path: &Path) -> BackendCompatibility;
}
