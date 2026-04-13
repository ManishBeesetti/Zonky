use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama as qlm;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::error::{Result, ZonkyError};
use crate::gpu::GpuDevice;
use crate::inference::{BackendCompatibility, InferenceBackend, ModelHandle};
use crate::types::*;

/// Internal state for a loaded quantized model
struct LoadedCandleModel {
    model: qlm::ModelWeights,
    tokenizer: tokenizers::Tokenizer,
    device: Device,
    model_id: String,
    file_size: u64,
}

/// Candle-based inference backend (pure Rust)
pub struct CandleBackend {
    models: Arc<RwLock<HashMap<String, LoadedCandleModel>>>,
}

impl CandleBackend {
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn gpu_device_to_candle(device: &GpuDevice) -> Device {
        match device {
            #[cfg(feature = "cuda")]
            GpuDevice::Cuda { index, .. } => {
                info!("Candle: using CUDA device {index}");
                Device::cuda_if_available(*index).unwrap_or(Device::Cpu)
            }
            #[cfg(feature = "metal")]
            GpuDevice::Metal { .. } => {
                info!("Candle: using Metal device");
                Device::new_metal(0).unwrap_or(Device::Cpu)
            }
            GpuDevice::Rocm { name, .. } => {
                warn!(
                    "Candle has no ROCm/HIP support — falling back to CPU for '{name}'. \
                     Use the llamacpp backend for AMD GPU acceleration."
                );
                Device::Cpu
            }
            _ => {
                info!("Candle: using CPU device");
                Device::Cpu
            }
        }
    }
}

impl Default for CandleBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for CandleBackend {
    fn name(&self) -> &str {
        "candle"
    }

    async fn load_model(
        &self,
        model_id: &str,
        path: &Path,
        device: &GpuDevice,
    ) -> Result<ModelHandle> {
        let candle_device = Self::gpu_device_to_candle(device);
        let model_path = path.to_path_buf();
        let model_id_owned = model_id.to_string();

        info!(model_id = %model_id, path = %path.display(), "Loading model with candle backend");

        // Detect format from extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let file_size = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        if ext != "gguf" {
            return Err(ZonkyError::InvalidModelFormat(
                format!("Candle backend currently supports GGUF format, got .{ext}")
            ));
        }

        // Load GGUF model
        let device_clone = candle_device.clone();
        let path_clone = model_path.clone();
        let (model, tokenizer) = tokio::task::spawn_blocking(move || {
            load_gguf_model(&path_clone, &device_clone)
        })
        .await
        .map_err(|e| ZonkyError::InferenceError(format!("Task join error: {e}")))??;

        let loaded = LoadedCandleModel {
            model,
            tokenizer,
            device: candle_device,
            model_id: model_id_owned.clone(),
            file_size,
        };

        self.models.write().await.insert(model_id_owned.clone(), loaded);

        info!(model_id = %model_id, "Model loaded successfully");

        Ok(ModelHandle {
            id: model_id_owned,
            backend: "candle".to_string(),
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
        let temperature = request.temperature;
        let top_p = request.top_p;
        let max_tokens = request.max_tokens;
        let repeat_penalty = request.repetition_penalty;
        let seed = request.seed.unwrap_or(42);
        let handle_id = handle.id.clone();

        info!(
            model_id = %handle_id,
            prompt_chars = prompt.len(),
            max_tokens = max_tokens,
            temp = temperature,
            "Starting candle inference"
        );

        let models = self.models.clone();

        let (generated_text, prompt_len, completion_tokens) =
            tokio::task::spawn_blocking(move || -> Result<(String, u32, u32)> {
                let mut models = models.blocking_write();
                let loaded = models
                    .get_mut(&handle_id)
                    .ok_or_else(|| ZonkyError::ModelNotFound(handle_id.clone()))?;

                // Tokenize
                let encoding = loaded
                    .tokenizer
                    .encode(prompt.as_str(), true)
                    .map_err(|e| ZonkyError::TokenizerError(e.to_string()))?;
                let prompt_tokens = encoding.get_ids().to_vec();
                let prompt_len = prompt_tokens.len() as u32;

                let eos_token = resolve_eos_token(&loaded.tokenizer);

                // Build initial input tensor
                let mut tokens = prompt_tokens.clone();
                let mut logits_processor = LogitsProcessor::new(seed, Some(temperature), Some(top_p));
                let mut generated_tokens: Vec<u32> = Vec::new();

                for i in 0..max_tokens {
                    let context_size = if i == 0 { tokens.len() } else { 1 };
                    let start_pos = tokens.len().saturating_sub(context_size);
                    let input = Tensor::new(&tokens[start_pos..], &loaded.device)
                        .map_err(|e| ZonkyError::InferenceError(format!("Tensor error: {e}")))?
                        .unsqueeze(0)
                        .map_err(|e| ZonkyError::InferenceError(format!("Unsqueeze error: {e}")))?;

                    let logits = loaded
                        .model
                        .forward(&input, start_pos)
                        .map_err(|e| ZonkyError::InferenceError(format!("Forward pass error: {e}")))?;

                    let logits = logits
                        .squeeze(0)
                        .map_err(|e| ZonkyError::InferenceError(format!("Squeeze error: {e}")))?;

                    // Apply repetition penalty
                    let logits = if repeat_penalty != 1.0 && !tokens.is_empty() {
                        let start = tokens.len().saturating_sub(64);
                        candle_transformers::utils::apply_repeat_penalty(
                            &logits,
                            repeat_penalty as f32,
                            &tokens[start..],
                        )
                        .map_err(|e| ZonkyError::InferenceError(format!("Repeat penalty error: {e}")))?
                    } else {
                        logits
                    };

                    let next_token = logits_processor.sample(&logits)
                        .map_err(|e| ZonkyError::InferenceError(format!("Sampling error: {e}")))?;

                    // Check for EOS
                    if Some(next_token) == eos_token {
                        break;
                    }

                    tokens.push(next_token);
                    generated_tokens.push(next_token);
                }

                // Decode generated tokens
                let generated_text = loaded
                    .tokenizer
                    .decode(&generated_tokens, true)
                    .map_err(|e| ZonkyError::TokenizerError(format!("Decode error: {e}")))?;

                let completion_tokens = generated_tokens.len() as u32;
                Ok((generated_text, prompt_len, completion_tokens))
            })
            .await
            .map_err(|e| ZonkyError::InferenceError(format!("Task join error: {e}")))??;

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
        let prompt = format_chat_prompt(&request.messages);
        let gen_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let created = chrono::Utc::now().timestamp();
        let model_name = request.model.clone();
        let temperature = request.temperature;
        let top_p = request.top_p;
        let max_tokens = request.max_tokens;
        let repeat_penalty = request.repetition_penalty;
        let seed = request.seed.unwrap_or(42);
        let handle_id = handle.id.clone();

        let models = self.models.clone();

        // Send initial chunk with role
        let initial_chunk = StreamChunk {
            id: gen_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model_name.clone(),
            choices: vec![StreamChoice {
                index: 0,
                delta: Delta {
                    role: Some(Role::Assistant),
                    content: None,
                },
                finish_reason: None,
            }],
        };

        if tx.send(Ok(initial_chunk)).await.is_err() {
            return Ok(());
        }

        let tx_clone = tx.clone();
        let gen_id_clone = gen_id.clone();
        let model_name_clone = model_name.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut models = models.blocking_write();
            let loaded = models
                .get_mut(&handle_id)
                .ok_or_else(|| ZonkyError::ModelNotFound(handle_id.clone()))?;

            // Tokenize
            let encoding = loaded
                .tokenizer
                .encode(prompt.as_str(), true)
                .map_err(|e| ZonkyError::TokenizerError(e.to_string()))?;
            let prompt_tokens = encoding.get_ids().to_vec();

            let eos_token = resolve_eos_token(&loaded.tokenizer);

            let mut tokens = prompt_tokens;
            let mut logits_processor = LogitsProcessor::new(seed, Some(temperature), Some(top_p));
            let mut prev_text_len = 0usize;
            let mut generated_tokens: Vec<u32> = Vec::new();

            for i in 0..max_tokens {
                let context_size = if i == 0 { tokens.len() } else { 1 };
                let start_pos = tokens.len().saturating_sub(context_size);
                let input = Tensor::new(&tokens[start_pos..], &loaded.device)
                    .map_err(|e| ZonkyError::InferenceError(format!("Tensor error: {e}")))?
                    .unsqueeze(0)
                    .map_err(|e| ZonkyError::InferenceError(format!("Unsqueeze error: {e}")))?;

                let logits = loaded
                    .model
                    .forward(&input, start_pos)
                    .map_err(|e| ZonkyError::InferenceError(format!("Forward pass error: {e}")))?;

                let logits = logits
                    .squeeze(0)
                    .map_err(|e| ZonkyError::InferenceError(format!("Squeeze error: {e}")))?;

                let logits = if repeat_penalty != 1.0 && !tokens.is_empty() {
                    let start = tokens.len().saturating_sub(64);
                    candle_transformers::utils::apply_repeat_penalty(
                        &logits,
                        repeat_penalty as f32,
                        &tokens[start..],
                    )
                    .map_err(|e| ZonkyError::InferenceError(format!("Repeat penalty error: {e}")))?
                } else {
                    logits
                };

                let next_token = logits_processor.sample(&logits)
                    .map_err(|e| ZonkyError::InferenceError(format!("Sampling error: {e}")))?;

                if Some(next_token) == eos_token {
                    break;
                }

                tokens.push(next_token);
                generated_tokens.push(next_token);

                // Decode all generated tokens and emit new text delta
                if let Ok(full_text) = loaded.tokenizer.decode(&generated_tokens, true) {
                    if full_text.len() > prev_text_len {
                        let new_text = &full_text[prev_text_len..];
                        prev_text_len = full_text.len();

                        let chunk = StreamChunk {
                            id: gen_id_clone.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: model_name_clone.clone(),
                            choices: vec![StreamChoice {
                                index: 0,
                                delta: Delta {
                                    role: None,
                                    content: Some(new_text.to_string()),
                                },
                                finish_reason: None,
                            }],
                        };

                        if tx_clone.blocking_send(Ok(chunk)).is_err() {
                            return Ok(());
                        }
                    }
                }
            }

            // Send final chunk
            let final_chunk = StreamChunk {
                id: gen_id_clone,
                object: "chat.completion.chunk".to_string(),
                created,
                model: model_name_clone,
                choices: vec![StreamChoice {
                    index: 0,
                    delta: Delta {
                        role: None,
                        content: None,
                    },
                    finish_reason: Some("stop".to_string()),
                }],
            };

            let _ = tx_clone.blocking_send(Ok(final_chunk));
            Ok(())
        })
        .await
        .map_err(|e| ZonkyError::InferenceError(format!("Task join error: {e}")))??;

        Ok(())
    }

    async fn unload_model(&self, handle: &ModelHandle) -> Result<()> {
        let removed = self.models.write().await.remove(&handle.id);
        if removed.is_some() {
            info!(model_id = %handle.id, "Model unloaded from candle backend");
            Ok(())
        } else {
            Err(ZonkyError::ModelNotFound(handle.id.clone()))
        }
    }

    fn model_info(&self, handle: &ModelHandle) -> Option<ModelInfo> {
        // This is a sync method, so we try_read
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
            vram_usage: Some(loaded.file_size), // approximate
            loaded: true,
            backend: Some("candle".to_string()),
            compatible_backends: vec!["candle".to_string()],
        })
    }

    fn estimate_vram(&self, path: &Path) -> Result<u64> {
        let metadata = std::fs::metadata(path)?;
        // GGUF models use roughly file_size * 1.1 VRAM (model + KV cache overhead)
        Ok((metadata.len() as f64 * 1.1) as u64)
    }

    fn can_load(&self, path: &Path) -> BackendCompatibility {
        match probe_gguf_compatibility(path) {
            Ok(()) => BackendCompatibility {
                compatible: true,
                reason: None,
            },
            Err(reason) => BackendCompatibility {
                compatible: false,
                reason: Some(reason),
            },
        }
    }
}

/// Map GGUF dtype id to a human-readable name.
/// See: https://github.com/ggerganov/ggml/blob/master/include/ggml.h (ggml_type enum)
fn gguf_dtype_name(id: u32) -> &'static str {
    match id {
        0 => "F32", 1 => "F16",
        2 => "Q4_0", 3 => "Q4_1",
        6 => "Q5_0", 7 => "Q5_1",
        8 => "Q8_0", 9 => "Q8_1",
        10 => "Q2_K", 11 => "Q3_K", 12 => "Q4_K", 13 => "Q5_K", 14 => "Q6_K", 15 => "Q8_K",
        16 => "IQ2_XXS", 17 => "IQ2_XS", 18 => "IQ3_XXS", 19 => "IQ1_S", 20 => "IQ4_NL",
        21 => "IQ3_S", 22 => "IQ2_S", 23 => "IQ4_XS",
        24 => "I8", 25 => "I16", 26 => "I32", 27 => "I64", 28 => "F64",
        29 => "IQ1_M", 30 => "BF16",
        _ => "unknown",
    }
}

/// Lightweight probe: read GGUF header to check if candle can handle all tensor dtypes.
/// Candle's `GgmlDType::from_u32` rejects unsupported dtypes (IQ-series, etc.) during
/// `Content::read`, so a successful read guarantees all tensor types are supported.
/// Returns Ok(()) if compatible, Err(reason) if not.
fn probe_gguf_compatibility(path: &Path) -> std::result::Result<(), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "gguf" {
        return Err(format!("Candle backend only supports GGUF format, got .{ext}"));
    }

    // Pre-screen filename for vision/projector files
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if filename.contains("mmproj") || filename.contains("vision-encoder") || filename.contains("clip-") {
        return Err("Vision/projector file, not a language model".to_string());
    }

    // Open and parse GGUF header (metadata + tensor info, no weight data).
    // This will fail with "unknown dtype" if the file contains tensor types
    // candle doesn't support (IQ-series quantizations like IQ3_S, IQ2_XS, etc.).
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Cannot open file: {e}"))?;
    let content = candle_core::quantized::gguf_file::Content::read(&mut file)
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("unknown dtype") {
                // Extract the dtype number to give a precise name
                let dtype_name = err_str
                    .split("unknown dtype for tensor ")
                    .nth(1)
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .map(|id| gguf_dtype_name(id))
                    .unwrap_or("unknown");
                format!("Unsupported GGUF tensor type: {dtype_name}. \
                    Candle only supports standard quantizations (Q4_K_M, Q5_K_M, Q8_0, F16, BF16). \
                    IQ-series quantizations (IQ2, IQ3, IQ4) are not supported. \
                    Download a Q4_K_M or Q5_K_M variant instead.")
            } else {
                format!("Cannot read GGUF: {err_str}")
            }
        })?;

    // Check architecture — reject vision/encoder models
    let arch = content.metadata.get("general.architecture")
        .and_then(|v| v.to_string().ok())
        .cloned()
        .unwrap_or_default();
    if arch == "clip" || arch == "t5encoder" || arch == "mllama" {
        return Err(format!("Architecture '{arch}' is a vision/encoder model, not supported for text generation"));
    }

    Ok(())
}

/// Load a GGUF model file and extract/find its tokenizer
fn load_gguf_model(
    path: &Path,
    device: &Device,
) -> Result<(qlm::ModelWeights, tokenizers::Tokenizer)> {
    info!(path = %path.display(), "Loading GGUF model weights");

    // Pre-screen filename: reject mmproj / vision-encoder files before parsing
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if filename.contains("mmproj") || filename.contains("vision-encoder") || filename.contains("clip-") {
        return Err(ZonkyError::InvalidModelFormat(
            format!("'{}' is a vision/projector file, not a language model. \
            Download the main model GGUF file instead (e.g. the Q4_K_M or Q8_0 variant).",
            path.file_name().unwrap_or_default().to_string_lossy())
        ));
    }

    let mut file = std::fs::File::open(path)?;
    let mut model_content = candle_core::quantized::gguf_file::Content::read(&mut file)
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("unknown dtype") {
                let dtype_name = err_str
                    .split("unknown dtype for tensor ")
                    .nth(1)
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .map(|id| gguf_dtype_name(id))
                    .unwrap_or("unknown");
                ZonkyError::InvalidModelFormat(
                    format!("Unsupported GGUF tensor type '{}' in '{}'. \
                    Candle supports standard quantizations (Q4_K_M, Q5_K_M, Q8_0, F16, BF16) \
                    but not IQ-series (IQ2, IQ3, IQ4). \
                    Download a Q4_K_M or Q5_K_M variant instead.",
                    dtype_name, path.file_name().unwrap_or_default().to_string_lossy())
                )
            } else {
                ZonkyError::InferenceError(format!("Failed to read GGUF: {e}"))
            }
        })?;

    // Reject non-LLM GGUF files (e.g. mmproj vision encoders)
    if let Some(general_type) = model_content.metadata.get("general.type") {
        if let Ok(t) = general_type.to_string() {
            if t != "model" {
                return Err(ZonkyError::InvalidModelFormat(
                    format!("This GGUF file is a '{}' (type: {}), not a language model. \
                    It cannot be used for text generation. Download the main model GGUF file instead.",
                    path.file_name().unwrap_or_default().to_string_lossy(), t)
                ));
            }
        }
    }

    // Also reject based on architecture
    let arch = model_content.metadata.get("general.architecture")
        .and_then(|v| v.to_string().ok())
        .cloned()
        .unwrap_or_default();
    if arch == "clip" || arch == "t5encoder" || arch == "mllama" {
        return Err(ZonkyError::InvalidModelFormat(
            format!("This GGUF file uses the '{}' architecture, which is a vision/encoder model, \
            not a text generation model. Download the main model GGUF file instead.", arch)
        ));
    }

    // Detect architecture and remap metadata keys to llama.* prefix
    // candle's quantized_llama::ModelWeights::from_gguf hardcodes "llama.*" keys,
    // but many architectures (qwen2, mistral, gemma, phi, etc.) use different prefixes
    // with the same underlying structure.
    remap_metadata_to_llama(&mut model_content);

    let model = qlm::ModelWeights::from_gguf(model_content, &mut file, device)
        .map_err(|e| ZonkyError::InferenceError(format!("Failed to load model weights: {e}")))?;

    // Try to find tokenizer.json in the same directory as the model
    let model_dir = path.parent().unwrap_or(Path::new("."));
    let tokenizer_path = model_dir.join("tokenizer.json");

    let tokenizer = if tokenizer_path.exists() {
        tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| ZonkyError::TokenizerError(format!("Failed to load tokenizer: {e}")))?
    } else {
        // No tokenizer found — return error so caller can download it
        warn!("No tokenizer.json found at {}. Download the tokenizer first.", model_dir.display());
        return Err(ZonkyError::TokenizerError(
            format!("No tokenizer.json found in {}. Use `zonky pull` to download the model with its tokenizer.", model_dir.display())
        ));
    };

    info!("Model and tokenizer loaded successfully");
    Ok((model, tokenizer))
}

/// Remap GGUF metadata keys from the model's native architecture prefix to `llama.*`
/// so that candle's `quantized_llama::ModelWeights::from_gguf` can load any compatible architecture.
fn remap_metadata_to_llama(content: &mut candle_core::quantized::gguf_file::Content) {
    use candle_core::quantized::gguf_file::Value;

    let arch = content
        .metadata
        .get("general.architecture")
        .and_then(|v| v.to_string().ok())
        .cloned()
        .unwrap_or_default();

    if arch.is_empty() || arch == "llama" {
        // Even for llama, check if rope.dimension_count needs synthesis
        synthesize_rope_dim(content, "llama");
        return;
    }

    info!(architecture = %arch, "Remapping GGUF metadata keys from '{arch}.*' to 'llama.*'");

    // Collect keys that need remapping (arch.* -> llama.*)
    let prefix = format!("{arch}.");
    let remapped: Vec<(String, Value)> = content
        .metadata
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(k, v)| {
            let new_key = format!("llama.{}", &k[prefix.len()..]);
            (new_key, v.clone())
        })
        .collect();

    for (key, value) in remapped {
        content.metadata.entry(key).or_insert(value);
    }

    // Synthesize rope.dimension_count if missing (common in Qwen/MoE models)
    synthesize_rope_dim(content, &arch);
}

/// Synthesize `llama.rope.dimension_count` from available metadata if missing.
/// Many architectures (Qwen, MoE variants) use `rope.dimension_sections` or
/// `attention.key_length` instead of the flat `rope.dimension_count` that candle expects.
fn synthesize_rope_dim(content: &mut candle_core::quantized::gguf_file::Content, arch: &str) {
    use candle_core::quantized::gguf_file::Value;

    if content.metadata.contains_key("llama.rope.dimension_count") {
        return;
    }

    // Try to compute from key_length (used by Qwen and others)
    let key_length = content
        .metadata
        .get(&format!("{arch}.attention.key_length"))
        .or_else(|| content.metadata.get("llama.attention.key_length"))
        .and_then(|v| v.to_u32().ok());

    if let Some(kl) = key_length {
        info!("Synthesizing llama.rope.dimension_count = {kl} from attention.key_length");
        content.metadata.insert(
            "llama.rope.dimension_count".to_string(),
            Value::U32(kl),
        );
        return;
    }

    // Fallback: compute as embedding_length / head_count
    let emb_len = content
        .metadata
        .get("llama.embedding_length")
        .and_then(|v| v.to_u32().ok());
    let head_count = content
        .metadata
        .get("llama.attention.head_count")
        .and_then(|v| v.to_u32().ok());

    if let (Some(e), Some(h)) = (emb_len, head_count) {
        if h > 0 {
            let dim = e / h;
            info!("Synthesizing llama.rope.dimension_count = {dim} from embedding_length/head_count");
            content.metadata.insert(
                "llama.rope.dimension_count".to_string(),
                Value::U32(dim),
            );
        }
    }
}

/// Resolve the EOS token ID from the tokenizer
fn resolve_eos_token(tokenizer: &tokenizers::Tokenizer) -> Option<u32> {
    // Try common EOS tokens: <|im_end|>, </s>, <|endoftext|>, <|end|>
    for candidate in &["<|im_end|>", "</s>", "<|endoftext|>", "<|end|>"] {
        if let Some(id) = tokenizer.token_to_id(candidate) {
            return Some(id);
        }
    }
    None
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
