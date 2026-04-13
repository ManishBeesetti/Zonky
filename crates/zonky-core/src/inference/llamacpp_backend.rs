use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::error::{Result, ZonkyError};
use crate::gpu::GpuDevice;
use crate::inference::{BackendCompatibility, InferenceBackend, ModelHandle};
use crate::types::*;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

/// Internal state for a loaded llama.cpp model
#[allow(dead_code)]
struct LoadedLlamaCppModel {
    model: Arc<LlamaModel>,
    model_id: String,
    file_size: u64,
    gpu_layers: u32,
}

// Safety: LlamaModel is thread-safe for read operations (loading/tokenizing)
// Write operations (context creation, inference) are synchronized by the RwLock.
unsafe impl Send for LoadedLlamaCppModel {}
unsafe impl Sync for LoadedLlamaCppModel {}

/// llama.cpp-based inference backend (supports CUDA, ROCm/HIP, Vulkan, Metal, CPU)
pub struct LlamaCppBackend {
    models: Arc<RwLock<HashMap<String, LoadedLlamaCppModel>>>,
    backend: Arc<std::sync::Mutex<Option<LlamaBackend>>>,
}

impl LlamaCppBackend {
    pub fn new() -> Self {
        // Initialize llama.cpp backend (must happen once)
        let backend = match LlamaBackend::init() {
            Ok(b) => {
                info!(gpu_offload = b.supports_gpu_offload(), "llama.cpp backend initialized");
                Some(b)
            }
            Err(e) => {
                warn!("llama.cpp backend init failed (may already be initialized): {e}");
                None
            }
        };

        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            backend: Arc::new(std::sync::Mutex::new(backend)),
        }
    }

    fn gpu_layers_for_device(device: &GpuDevice) -> u32 {
        match device {
            GpuDevice::Cpu => 0,
            // Full GPU offload for all GPU types
            _ => 999,
        }
    }
}

impl Default for LlamaCppBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for LlamaCppBackend {
    fn name(&self) -> &str {
        "llamacpp"
    }

    async fn load_model(
        &self,
        model_id: &str,
        path: &Path,
        device: &GpuDevice,
    ) -> Result<ModelHandle> {
        let model_path = path.to_path_buf();
        let model_id_owned = model_id.to_string();
        let gpu_layers = Self::gpu_layers_for_device(device);

        info!(
            model_id = %model_id,
            path = %path.display(),
            gpu_layers = gpu_layers,
            device = %device.device_name(),
            "Loading model with llama.cpp backend"
        );

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        let backend_ref = self.backend.clone();
        let (model, actual_gpu_layers) = tokio::task::spawn_blocking(move || -> Result<(LlamaModel, u32)> {
            let backend_guard = backend_ref.lock().unwrap();
            let backend = backend_guard.as_ref()
                .ok_or_else(|| ZonkyError::BackendError("llama.cpp backend not initialized".to_string()))?;

            let model_params = LlamaModelParams::default()
                .with_n_gpu_layers(gpu_layers);

            let model = LlamaModel::load_from_file(backend, &model_path, &model_params)
                .map_err(|e| ZonkyError::InferenceError(format!("llama.cpp model load failed: {e}")))?;

            Ok((model, gpu_layers))
        })
        .await
        .map_err(|e| ZonkyError::InferenceError(format!("Task join error: {e}")))?
        ?;

        let loaded = LoadedLlamaCppModel {
            model: Arc::new(model),
            model_id: model_id_owned.clone(),
            file_size,
            gpu_layers: actual_gpu_layers,
        };

        self.models.write().await.insert(model_id_owned.clone(), loaded);

        info!(model_id = %model_id, gpu_layers = actual_gpu_layers, "Model loaded successfully via llama.cpp");

        Ok(ModelHandle {
            id: model_id_owned,
            backend: "llamacpp".to_string(),
        })
    }

    async fn generate(
        &self,
        handle: &ModelHandle,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse> {
        let prompt = format_chat_prompt(&request.messages);
        let gen_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let created = chrono::Utc::now().timestamp();
        let model_name = request.model.clone();
        let max_tokens = request.max_tokens;
        let temperature = request.temperature as f32;
        let top_p = request.top_p as f32;
        let repeat_penalty = request.repetition_penalty as f32;
        let handle_id = handle.id.clone();

        let models = self.models.clone();
        let backend_ref = self.backend.clone();

        info!(
            model_id = %handle_id,
            prompt_len = prompt.len(),
            max_tokens = max_tokens,
            "Starting llama.cpp inference"
        );

        let (generated_text, prompt_len, completion_tokens) =
            tokio::task::spawn_blocking(move || -> Result<(String, u32, u32)> {
                let models = models.blocking_read();
                let loaded = models
                    .get(&handle_id)
                    .ok_or_else(|| ZonkyError::ModelNotFound(handle_id.clone()))?;

                // Create context — need backend reference
                let backend_guard = backend_ref.lock().unwrap();
                let backend = backend_guard.as_ref()
                    .ok_or_else(|| ZonkyError::BackendError("llama.cpp backend not initialized".to_string()))?;
                let ctx_params = LlamaContextParams::default()
                    .with_n_ctx(std::num::NonZeroU32::new(4096));
                let mut ctx = loaded.model
                    .new_context(backend, ctx_params)
                    .map_err(|e| ZonkyError::InferenceError(format!("Context creation failed: {e}")))?;

                // Tokenize prompt
                let prompt_tokens = loaded.model
                    .str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
                    .map_err(|e| ZonkyError::TokenizerError(format!("Tokenization failed: {e}")))?;
                let prompt_len = prompt_tokens.len() as u32;

                info!(prompt_tokens = prompt_len, "Tokenized prompt for llama.cpp");

                // Process prompt in batch
                let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(4096, 1);
                let last_idx = prompt_tokens.len() - 1;
                for (i, token) in prompt_tokens.iter().enumerate() {
                    let is_last = i == last_idx;
                    batch.add(*token, i as i32, &[0], is_last)
                        .map_err(|e| ZonkyError::InferenceError(format!("Batch add failed: {e}")))?;
                }

                ctx.decode(&mut batch)
                    .map_err(|e| ZonkyError::InferenceError(format!("Prompt decode failed: {e}")))?;

                info!("Prompt processed, starting generation");

                // Build sampler chain
                let mut sampler = LlamaSampler::chain_simple([
                    LlamaSampler::temp(temperature),
                    LlamaSampler::top_p(top_p, 1),
                    LlamaSampler::penalties(64, repeat_penalty, 0.0, 0.0),
                    LlamaSampler::dist(42),
                ]);

                // Generate tokens
                let mut generated_tokens = Vec::new();
                let mut n_cur = prompt_tokens.len();
                let _eos_token = loaded.model.token_eos();

                for i in 0..max_tokens {
                    let token = sampler.sample(&ctx, -1);

                    // Use is_eog_token for robust end-of-generation detection
                    if loaded.model.is_eog_token(token) {
                        debug!(tokens_generated = i, "Hit EOG token");
                        break;
                    }

                    generated_tokens.push(token);

                    // Log progress periodically
                    if (i + 1) % 50 == 0 {
                        debug!(tokens = i + 1, "Generation progress");
                    }

                    // Prepare next batch
                    batch.clear();
                    batch.add(token, n_cur as i32, &[0], true)
                        .map_err(|e| ZonkyError::InferenceError(format!("Batch add failed: {e}")))?;
                    n_cur += 1;

                    ctx.decode(&mut batch)
                        .map_err(|e| ZonkyError::InferenceError(format!("Decode failed at token {i}: {e}")))?;
                }

                // Decode tokens to text
                let mut output_bytes: Vec<u8> = Vec::new();
                for t in &generated_tokens {
                    let piece = loaded.model.token_to_piece_bytes(*t, 32, true, None)
                        .map_err(|e| ZonkyError::InferenceError(format!("Detokenize failed: {e}")))?;
                    output_bytes.extend_from_slice(&piece);
                }
                let generated_text = String::from_utf8_lossy(&output_bytes).to_string();

                // Strip ChatML control tokens that may leak into the output
                let generated_text = strip_chat_artifacts(&generated_text);

                let completion_tokens = generated_tokens.len() as u32;
                info!(completion_tokens = completion_tokens, "Generation complete");

                Ok((generated_text, prompt_len, completion_tokens))
            })
            .await
            .map_err(|e| ZonkyError::InferenceError(format!("Task join error: {e}")))?
            ?;

        let response = GenerationResponse {
            id: gen_id,
            object: "chat.completion".to_string(),
            created,
            model: model_name,
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: Role::Assistant,
                    content: generated_text,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage {
                prompt_tokens: prompt_len,
                completion_tokens,
                total_tokens: prompt_len + completion_tokens,
            },
        };

        Ok(response)
    }

    async fn generate_stream(
        &self,
        handle: &ModelHandle,
        request: &GenerationRequest,
        tx: mpsc::Sender<Result<StreamChunk>>,
    ) -> Result<()> {
        // For now, generate full response and send as single chunk
        // TODO: implement true streaming
        let response = self.generate(handle, request).await?;
        let content = response.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let chunk = StreamChunk {
            id: response.id,
            object: "chat.completion.chunk".to_string(),
            created: response.created,
            model: response.model,
            choices: vec![StreamChoice {
                index: 0,
                delta: Delta {
                    role: Some(Role::Assistant),
                    content: Some(content),
                },
                finish_reason: Some("stop".to_string()),
            }],
        };

        let _ = tx.send(Ok(chunk)).await;
        Ok(())
    }

    async fn unload_model(&self, handle: &ModelHandle) -> Result<()> {
        let removed = self.models.write().await.remove(&handle.id);
        if removed.is_some() {
            info!(model_id = %handle.id, "Model unloaded from llama.cpp backend");
            Ok(())
        } else {
            Err(ZonkyError::ModelNotFound(handle.id.clone()))
        }
    }

    fn model_info(&self, handle: &ModelHandle) -> Option<ModelInfo> {
        let models = self.models.try_read().ok()?;
        let loaded = models.get(&handle.id)?;

        Some(ModelInfo {
            id: loaded.model_id.clone(),
            object: "model".to_string(),
            created: 0,
            owned_by: "local".to_string(),
            architecture: None,
            quantization: Some("GGUF".to_string()),
            parameters: None,
            file_size: Some(loaded.file_size),
            vram_usage: Some(loaded.file_size),
            loaded: true,
            backend: Some("llamacpp".to_string()),
            compatible_backends: vec!["llamacpp".to_string()],
        })
    }

    fn estimate_vram(&self, path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path)?;
        Ok((metadata.len() as f64 * 1.1) as u64)
    }

    fn can_load(&self, path: &Path) -> BackendCompatibility {
        // llama.cpp supports all GGUF files including IQ quantizations
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "gguf" {
            BackendCompatibility {
                compatible: true,
                reason: None,
            }
        } else {
            BackendCompatibility {
                compatible: false,
                reason: Some(format!("Not a GGUF file (.{ext})")),
            }
        }
    }
}

/// Strip ChatML control tokens and role prefixes that leak into generated text
fn strip_chat_artifacts(text: &str) -> String {
    let mut cleaned = text.to_string();
    // Remove ChatML control tokens
    for tag in &[
        "<|im_start|>", "<|im_end|>",
        "<|start_header_id|>", "<|end_header_id|>",
        "<|eot_id|>", "<|begin_of_text|>",
    ] {
        cleaned = cleaned.replace(tag, "");
    }
    // Remove role prefixes that appear after control tokens
    for role in &["assistant\n", "user\n", "system\n", "assistant", "user", "system"] {
        // Only strip if it appears at the very start or right after a newline from a tag removal
        if cleaned.starts_with(role) {
            cleaned = cleaned[role.len()..].to_string();
        }
    }
    cleaned.trim().to_string()
}

/// Format chat messages into a prompt string (ChatML format)
fn format_chat_prompt(messages: &[Message]) -> String {
    let mut prompt = String::new();
    for msg in messages {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        prompt.push_str(&format!("<|im_start|>{role}\n{}<|im_end|>\n", msg.content));
    }
    prompt.push_str("<|im_start|>assistant\n");
    prompt
}
