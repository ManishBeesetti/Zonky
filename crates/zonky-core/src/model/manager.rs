use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::error::{Result, ZonkyError};
use crate::gpu::{self, GpuDevice};
use crate::hub::cache::extract_quantization;
use crate::hub::HubClient;
use crate::inference::candle_backend::CandleBackend;
#[cfg(feature = "llamacpp")]
use crate::inference::llamacpp_backend::LlamaCppBackend;
use crate::inference::{InferenceBackend, ModelHandle};
use crate::types::*;

const MAX_TELEMETRY_RECORDS: usize = 200;

/// A loaded model with its metadata and backend reference
struct LoadedModel {
    handle: ModelHandle,
    info: ModelInfo,
    backend_name: String,
    device_name: String,
    last_used: Instant,
    vram_usage: u64,
}

/// Manages multiple loaded models with VRAM budget tracking
pub struct ModelManager {
    /// Loaded models indexed by model ID
    models: Arc<RwLock<HashMap<String, LoadedModel>>>,
    /// Available inference backends
    backends: HashMap<String, Box<dyn InferenceBackend>>,
    /// Hub client for model resolution
    hub: HubClient,
    /// Configuration
    config: Arc<RwLock<super::config::ZonkyConfig>>,
    /// Available GPU devices
    devices: Vec<GpuDevice>,
    /// Active generation cancellation tokens
    active_generations: Arc<RwLock<Vec<Arc<AtomicBool>>>>,
    /// Recent inference telemetry records
    telemetry: Arc<RwLock<Vec<TelemetryRecord>>>,
}

impl ModelManager {
    pub fn new(config: super::config::ZonkyConfig) -> Result<Self> {
        let mut backends: HashMap<String, Box<dyn InferenceBackend>> = HashMap::new();

        // Register both inference backends
        backends.insert("candle".to_string(), Box::new(CandleBackend::new()));
        #[cfg(feature = "llamacpp")]
        backends.insert("llamacpp".to_string(), Box::new(LlamaCppBackend::new()));

        let cache_dir = config.cache_dir();
        let hub = HubClient::new(cache_dir.clone())?;
        let devices = gpu::detect_devices();
        let initial_telemetry = load_telemetry_from_disk(&cache_dir);

        info!(
            backends = ?backends.keys().collect::<Vec<_>>(),
            devices = ?devices.iter().map(|d| d.device_name()).collect::<Vec<_>>(),
            "ModelManager initialized"
        );

        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            backends,
            hub,
            config: Arc::new(RwLock::new(config)),
            devices,
            active_generations: Arc::new(RwLock::new(Vec::new())),
            telemetry: Arc::new(RwLock::new(initial_telemetry)),
        })
    }

    /// Get current configuration snapshot
    pub async fn get_config(&self) -> super::config::ZonkyConfig {
        self.config.read().await.clone()
    }

    /// Update configuration in memory and persist to disk
    pub async fn update_config(&self, new_config: super::config::ZonkyConfig) -> Result<()> {
        new_config.save()?;
        *self.config.write().await = new_config;
        Ok(())
    }

    /// Cancel all active generation requests and return how many were signalled
    pub async fn cancel_generation(&self) -> usize {
        let mut tokens = self.active_generations.write().await;
        let count = tokens.len();
        for token in tokens.iter() {
            token.store(true, Ordering::Relaxed);
        }
        tokens.clear();
        count
    }

    /// Record a telemetry entry and persist to disk
    pub async fn record_telemetry(&self, record: TelemetryRecord) {
        let cache_dir = self.hub.cache_dir().to_path_buf();
        let mut history = self.telemetry.write().await;
        history.push(record);
        if history.len() > MAX_TELEMETRY_RECORDS {
            let drain_count = history.len() - MAX_TELEMETRY_RECORDS;
            history.drain(0..drain_count);
        }
        let snapshot = history.clone();
        drop(history);
        save_telemetry_to_disk(&cache_dir, &snapshot);
    }

    /// Get aggregated telemetry summary and recent run history
    pub async fn get_telemetry(&self) -> TelemetrySummary {
        let history = self.telemetry.read().await;
        summarize_telemetry(&history)
    }

    /// Clear recorded telemetry history
    pub async fn clear_telemetry(&self) {
        let cache_dir = self.hub.cache_dir().to_path_buf();
        let mut history = self.telemetry.write().await;
        history.clear();
        drop(history);
        save_telemetry_to_disk(&cache_dir, &[]);
    }

    /// Recommend and score GGUF files from a repository based on current hardware
    pub fn recommend_files(&self, files: &[HubSibling]) -> Vec<FileRecommendation> {
        let (_, ram_free) = gpu::system_memory_bytes();
        recommend_gguf_files(files, &self.devices, ram_free)
    }

    /// Get the best available device
    pub fn best_device(&self) -> GpuDevice {
        self.devices
            .iter()
            .find(|d| d.is_gpu())
            .cloned()
            .unwrap_or(GpuDevice::Cpu)
    }

    /// Load a model by its local ID or path.
    /// When `BackendChoice::Auto`, tries each compatible backend in order,
    /// falling back to the next if one fails.
    pub async fn load_model(
        &self,
        model_id: &str,
        backend_choice: BackendChoice,
        device_choice: DeviceChoice,
    ) -> Result<()> {
        // Check if already loaded
        if self.models.read().await.contains_key(model_id) {
            return Err(ZonkyError::ModelAlreadyLoaded(model_id.to_string()));
        }

        let cfg = self.config.read().await.clone();

        // Enforce max_loaded_models limit
        {
            let loaded_count = self.models.read().await.len();
            if loaded_count >= cfg.max_loaded_models {
                if cfg.auto_evict {
                    self.evict_oldest_one().await?;
                } else {
                    return Err(ZonkyError::BackendError(format!(
                        "Maximum loaded models limit reached ({})",
                        cfg.max_loaded_models
                    )));
                }
            }
        }

        // Resolve model path
        let model_path = self.resolve_model_path(model_id)?;

        // Select device
        let device = match device_choice {
            DeviceChoice::Auto => self.best_device(),
            DeviceChoice::Cpu => GpuDevice::Cpu,
            DeviceChoice::Cuda(idx) => self
                .devices
                .iter()
                .find(|d| matches!(d, GpuDevice::Cuda { index, .. } if *index == idx))
                .cloned()
                .ok_or_else(|| ZonkyError::GpuError(format!("CUDA device {idx} not found")))?,
            DeviceChoice::Metal => self
                .devices
                .iter()
                .find(|d| matches!(d, GpuDevice::Metal { .. }))
                .cloned()
                .ok_or_else(|| ZonkyError::GpuError("Metal device not found".to_string()))?,
        };

        // Build ordered list of backends to try
        let backend_order: Vec<String> = match backend_choice {
            BackendChoice::Candle => vec!["candle".to_string()],
            BackendChoice::LlamaCpp => vec!["llamacpp".to_string()],
            BackendChoice::Auto => self.backend_priority_order(&model_path),
        };

        // Try each backend in order, with fallback
        let mut errors: Vec<(String, String)> = Vec::new();

        for backend_name in &backend_order {
            let Some(backend) = self.backends.get(backend_name) else {
                errors.push((backend_name.clone(), "Backend not available".to_string()));
                continue;
            };

            // Quick compatibility probe
            let compat = backend.can_load(&model_path);
            if !compat.compatible {
                let reason = compat.reason.unwrap_or_else(|| "Incompatible".to_string());
                warn!(
                    model_id = %model_id, backend = %backend_name,
                    reason = %reason, "Backend cannot load model, trying next"
                );
                errors.push((backend_name.clone(), reason));
                continue;
            }

            // Check VRAM budget
            let estimated_vram = backend.estimate_vram(&model_path)?;
            if device.is_gpu() {
                let available = device.vram_free();
                let used: u64 = self
                    .models
                    .read()
                    .await
                    .values()
                    .map(|m| m.vram_usage)
                    .sum();
                let free = available.saturating_sub(used);

                if estimated_vram > free {
                    if cfg.auto_evict {
                        self.evict_lru(estimated_vram - free).await?;
                    } else {
                        return Err(ZonkyError::InsufficientVram {
                            needed: estimated_vram,
                            available: free,
                        });
                    }
                }
            }

            // Attempt to load
            info!(model_id = %model_id, backend = %backend_name, device = %device.device_name(), "Loading model");
            match backend.load_model(model_id, &model_path, &device).await {
                Ok(handle) => {
                    let estimated_vram = backend.estimate_vram(&model_path).unwrap_or(0);
                    let model_info = backend.model_info(&handle).unwrap_or_else(|| ModelInfo {
                        id: model_id.to_string(),
                        object: "model".to_string(),
                        created: chrono::Utc::now().timestamp(),
                        owned_by: "local".to_string(),
                        architecture: None,
                        quantization: None,
                        parameters: None,
                        file_size: None,
                        vram_usage: Some(estimated_vram),
                        loaded: true,
                        backend: Some(backend_name.clone()),
                        compatible_backends: vec![],
                    });

                    let loaded = LoadedModel {
                        handle,
                        info: model_info,
                        backend_name: backend_name.clone(),
                        device_name: device.device_name(),
                        last_used: Instant::now(),
                        vram_usage: estimated_vram,
                    };

                    self.models
                        .write()
                        .await
                        .insert(model_id.to_string(), loaded);
                    info!(model_id = %model_id, backend = %backend_name, "Model loaded successfully");
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        model_id = %model_id, backend = %backend_name,
                        error = %e, "Backend failed to load model, trying next"
                    );
                    errors.push((backend_name.clone(), e.to_string()));
                    continue;
                }
            }
        }

        // All backends failed
        let detail = errors
            .iter()
            .map(|(b, e)| format!("  {b}: {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        Err(ZonkyError::BackendError(format!(
            "Failed to load model '{model_id}' — all backends failed:\n{detail}"
        )))
    }

    /// Unload a model
    pub async fn unload_model(&self, model_id: &str) -> Result<()> {
        let loaded = self
            .models
            .write()
            .await
            .remove(model_id)
            .ok_or_else(|| ZonkyError::ModelNotFound(model_id.to_string()))?;

        if let Some(backend) = self.backends.get(&loaded.backend_name) {
            backend.unload_model(&loaded.handle).await?;
        }

        info!(model_id = %model_id, "Model unloaded");
        Ok(())
    }

    /// Generate a complete response
    pub async fn generate(
        &self,
        model_id: &str,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse> {
        let (handle, backend_name, device_name) = {
            let mut models = self.models.write().await;
            let loaded = models
                .get_mut(model_id)
                .ok_or_else(|| ZonkyError::ModelNotFound(model_id.to_string()))?;
            loaded.last_used = Instant::now();
            (
                loaded.handle.clone(),
                loaded.backend_name.clone(),
                loaded.device_name.clone(),
            )
        };

        let backend = self
            .backends
            .get(&backend_name)
            .ok_or_else(|| ZonkyError::BackendError(backend_name.clone()))?;

        let cancel_token = request
            .cancel_token
            .clone()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        self.active_generations
            .write()
            .await
            .push(cancel_token.clone());

        let mut req_with_token = request.clone();
        req_with_token.cancel_token = Some(cancel_token.clone());

        let start = Instant::now();
        let result = backend.generate(&handle, &req_with_token).await;

        self.active_generations
            .write()
            .await
            .retain(|t| !Arc::ptr_eq(t, &cancel_token));

        let mut response = result?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let tokens_per_second = if duration_ms > 0 {
            (response.usage.completion_tokens as f64) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let record = TelemetryRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            model_id: model_id.to_string(),
            backend: backend_name,
            device: device_name,
            prompt_tokens: response.usage.prompt_tokens,
            completion_tokens: response.usage.completion_tokens,
            duration_ms,
            tokens_per_second,
            time_to_first_token_ms: None,
        };
        response.telemetry = Some(record.clone());
        self.record_telemetry(record).await;

        Ok(response)
    }

    /// Generate a streaming response
    pub async fn generate_stream(
        &self,
        model_id: &str,
        request: &GenerationRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let (handle, backend_name, device_name) = {
            let mut models = self.models.write().await;
            let loaded = models
                .get_mut(model_id)
                .ok_or_else(|| ZonkyError::ModelNotFound(model_id.to_string()))?;
            loaded.last_used = Instant::now();
            (
                loaded.handle.clone(),
                loaded.backend_name.clone(),
                loaded.device_name.clone(),
            )
        };

        let backend = self
            .backends
            .get(&backend_name)
            .ok_or_else(|| ZonkyError::BackendError(backend_name.clone()))?;

        let cancel_token = request
            .cancel_token
            .clone()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        self.active_generations
            .write()
            .await
            .push(cancel_token.clone());

        let mut req_with_token = request.clone();
        req_with_token.cancel_token = Some(cancel_token.clone());

        let (backend_tx, mut backend_rx) = mpsc::channel(64);
        let (out_tx, out_rx) = mpsc::channel(64);

        backend
            .generate_stream(&handle, &req_with_token, backend_tx)
            .await?;

        let active_generations = self.active_generations.clone();
        let telemetry_store = self.telemetry.clone();
        let cache_dir = self.hub.cache_dir().to_path_buf();
        let model_id_owned = model_id.to_string();
        let prompt_chars: usize = request.messages.iter().map(|m| m.content.len()).sum();
        let prompt_tokens_est = (prompt_chars / 4).max(1) as u32;
        let start = Instant::now();

        tokio::spawn(async move {
            let mut completion_tokens: u32 = 0;
            let mut ttft_ms: Option<u64> = None;

            while let Some(item) = backend_rx.recv().await {
                if let Ok(ref chunk) = item {
                    if !chunk.delta_text().is_empty() {
                        completion_tokens += 1;
                        if ttft_ms.is_none() {
                            ttft_ms = Some(start.elapsed().as_millis() as u64);
                        }
                    }
                }
                if out_tx.send(item).await.is_err() {
                    cancel_token.store(true, Ordering::Relaxed);
                    break;
                }
            }

            active_generations
                .write()
                .await
                .retain(|t| !Arc::ptr_eq(t, &cancel_token));

            if completion_tokens > 0 {
                let duration_ms = start.elapsed().as_millis() as u64;
                let tokens_per_second = if duration_ms > 0 {
                    (completion_tokens as f64) / (duration_ms as f64 / 1000.0)
                } else {
                    0.0
                };
                let record = TelemetryRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    model_id: model_id_owned,
                    backend: backend_name,
                    device: device_name,
                    prompt_tokens: prompt_tokens_est,
                    completion_tokens,
                    duration_ms,
                    tokens_per_second,
                    time_to_first_token_ms: ttft_ms,
                };
                let mut history = telemetry_store.write().await;
                history.push(record);
                if history.len() > MAX_TELEMETRY_RECORDS {
                    let drain_count = history.len() - MAX_TELEMETRY_RECORDS;
                    history.drain(0..drain_count);
                }
                let snapshot = history.clone();
                drop(history);
                save_telemetry_to_disk(&cache_dir, &snapshot);
            }
        });

        Ok(out_rx)
    }

    /// List all loaded models
    pub async fn list_loaded_models(&self) -> Vec<ModelInfo> {
        self.models
            .read()
            .await
            .values()
            .map(|m| m.info.clone())
            .collect()
    }

    /// List all available models (local cache + loaded)
    pub async fn list_all_models(&self) -> Result<Vec<ModelInfo>> {
        let local_models = self.hub.list_local_models()?;
        let loaded = self.models.read().await;

        let models: Vec<ModelInfo> = local_models
            .iter()
            .map(|lm| {
                let is_loaded = loaded.contains_key(&lm.id);
                let loaded_info = loaded.get(&lm.id);
                let compatible = self.probe_compatible_backends(&lm.path);

                ModelInfo {
                    id: lm.id.clone(),
                    object: "model".to_string(),
                    created: 0,
                    owned_by: "local".to_string(),
                    architecture: lm.architecture.clone(),
                    quantization: lm.quantization.clone(),
                    parameters: None,
                    file_size: Some(lm.file_size),
                    vram_usage: loaded_info.map(|l| l.vram_usage),
                    loaded: is_loaded,
                    backend: loaded_info.map(|l| l.backend_name.clone()),
                    compatible_backends: compatible,
                }
            })
            .collect();

        Ok(models)
    }

    /// Get the hub client reference
    pub fn hub(&self) -> &HubClient {
        &self.hub
    }

    /// Get available devices
    pub fn devices(&self) -> &[GpuDevice] {
        &self.devices
    }

    /// Resolve a model ID to a file path
    fn resolve_model_path(&self, model_id: &str) -> Result<PathBuf> {
        // First check if it's a direct path
        let path = PathBuf::from(model_id);
        if path.exists() {
            return Ok(path);
        }

        // Try to find in local cache by ID or alias
        if let Some(local) = self.hub.list_local_models()?.iter().find(|m| {
            m.id == model_id || m.alias.as_deref() == Some(model_id) || m.repo_id == model_id
        }) {
            if local.path.exists() {
                return Ok(local.path.clone());
            }
        }

        Err(ZonkyError::ModelNotFound(format!(
            "Cannot find model '{model_id}'. Use `zonky pull` to download it first."
        )))
    }

    /// Build a priority-ordered list of backends based on detected hardware.
    /// - AMD ROCm GPU → llamacpp first (candle has no ROCm support)
    /// - NVIDIA CUDA GPU → candle first (Rust-native), llamacpp second
    /// - Apple Metal → candle first (native Metal), llamacpp second
    /// - CPU only → candle first (lighter runtime), llamacpp second
    fn backend_priority_order(&self, _path: &PathBuf) -> Vec<String> {
        let has_rocm = self
            .devices
            .iter()
            .any(|d| matches!(d, GpuDevice::Rocm { .. }));
        let has_cuda = self
            .devices
            .iter()
            .any(|d| matches!(d, GpuDevice::Cuda { .. }));
        let has_metal = self
            .devices
            .iter()
            .any(|d| matches!(d, GpuDevice::Metal { .. }));

        let mut order = Vec::new();

        if has_rocm {
            // AMD ROCm: llamacpp first (has HIP/ROCm GPU support), candle second (CPU-only fallback)
            info!("Hardware-aware selection: AMD ROCm detected → preferring llamacpp backend");
            if self.backends.contains_key("llamacpp") {
                order.push("llamacpp".to_string());
            }
            if self.backends.contains_key("candle") {
                order.push("candle".to_string());
            }
        } else if has_cuda {
            // NVIDIA CUDA: candle first (Rust-native CUDA), llamacpp second
            if self.backends.contains_key("candle") {
                order.push("candle".to_string());
            }
            if self.backends.contains_key("llamacpp") {
                order.push("llamacpp".to_string());
            }
        } else if has_metal {
            // Apple Metal: candle first (native Metal), llamacpp second
            if self.backends.contains_key("candle") {
                order.push("candle".to_string());
            }
            if self.backends.contains_key("llamacpp") {
                order.push("llamacpp".to_string());
            }
        } else {
            // CPU only: candle first (lighter), llamacpp second
            if self.backends.contains_key("candle") {
                order.push("candle".to_string());
            }
            if self.backends.contains_key("llamacpp") {
                order.push("llamacpp".to_string());
            }
        }

        if order.is_empty() {
            order.extend(self.backends.keys().cloned());
        }
        order
    }

    /// Probe all registered backends for compatibility with a model file.
    /// Returns a list of backend names that can load the file.
    pub fn probe_compatible_backends(&self, path: &std::path::Path) -> Vec<String> {
        self.backends
            .iter()
            .filter(|(_, backend)| backend.can_load(path).compatible)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Evict the single oldest model when `max_loaded_models` is reached
    async fn evict_oldest_one(&self) -> Result<()> {
        let oldest_id = {
            let models = self.models.read().await;
            models
                .iter()
                .min_by_key(|(_, m)| m.last_used)
                .map(|(id, _)| id.clone())
        };
        if let Some(id) = oldest_id {
            warn!(model_id = %id, "Evicting oldest model to respect max_loaded_models");
            self.unload_model(&id).await?;
        }
        Ok(())
    }

    /// Evict least recently used models to free the requested amount of VRAM
    async fn evict_lru(&self, bytes_needed: u64) -> Result<()> {
        let mut freed: u64 = 0;
        let mut to_evict = Vec::new();

        {
            let models = self.models.read().await;
            let mut entries: Vec<_> = models.iter().collect();
            entries.sort_by_key(|(_, m)| m.last_used);

            for (id, model) in entries {
                if freed >= bytes_needed {
                    break;
                }
                to_evict.push(id.clone());
                freed += model.vram_usage;
            }
        }

        for id in to_evict {
            warn!(model_id = %id, "Evicting model to free VRAM");
            self.unload_model(&id).await?;
        }

        Ok(())
    }
}

/// Score and recommend GGUF files based on available GPU VRAM and system RAM
pub fn recommend_gguf_files(
    files: &[HubSibling],
    devices: &[GpuDevice],
    ram_free_bytes: u64,
) -> Vec<FileRecommendation> {
    let best_gpu = devices.iter().find(|d| d.is_gpu());
    let gpu_vram = best_gpu.map(|d| d.vram_free()).unwrap_or(0);
    let has_rocm = devices.iter().any(|d| matches!(d, GpuDevice::Rocm { .. }));
    let recommended_backend = if has_rocm {
        "llamacpp".to_string()
    } else {
        "candle".to_string()
    };

    // Estimate memory overhead (~20% for KV cache + runtime buffers)
    let mut recs: Vec<FileRecommendation> = files
        .iter()
        .map(|f| {
            let size = f.size.unwrap_or(0);
            let estimated_vram_bytes = ((size as f64) * 1.2) as u64;
            let quantization = extract_quantization(&f.filename);

            let (fit_level, reason) = if size == 0 {
                (
                    "good".to_string(),
                    "File size unknown; verify memory before loading".to_string(),
                )
            } else if gpu_vram > 0 && estimated_vram_bytes <= gpu_vram {
                (
                    "optimal".to_string(),
                    format!(
                        "Fits comfortably in GPU VRAM ({:.1} GB est.)",
                        estimated_vram_bytes as f64 / 1e9
                    ),
                )
            } else if ram_free_bytes > 0 && estimated_vram_bytes <= ram_free_bytes {
                if gpu_vram > 0 {
                    (
                        "good".to_string(),
                        "Exceeds GPU VRAM; can run on CPU or with partial offload".to_string(),
                    )
                } else {
                    (
                        "good".to_string(),
                        format!(
                            "Fits in system RAM ({:.1} GB est.)",
                            estimated_vram_bytes as f64 / 1e9
                        ),
                    )
                }
            } else if gpu_vram == 0 && ram_free_bytes == 0 {
                ("good".to_string(), "Compatible GGUF model".to_string())
            } else {
                (
                    "too_large".to_string(),
                    format!(
                        "Requires ~{:.1} GB, which exceeds available memory",
                        estimated_vram_bytes as f64 / 1e9
                    ),
                )
            };

            FileRecommendation {
                filename: f.filename.clone(),
                size: f.size,
                quantization,
                estimated_vram_bytes,
                fit_level,
                recommended: false,
                recommended_backend: recommended_backend.clone(),
                reason,
            }
        })
        .collect();

    // Pick the single best recommendation among fitting files (prefer Q4_K_M > Q5_K_M > Q4_K_S > Q8_0 > any fitting)
    let quant_rank = |q: Option<&str>| -> u8 {
        match q {
            Some("Q4_K_M") => 100,
            Some("Q5_K_M") => 95,
            Some("Q4_K_S") | Some("Q4_K_L") => 90,
            Some("Q5_K_S") => 85,
            Some("Q6_K") => 80,
            Some("Q8_0") => 75,
            Some("Q3_K_M") | Some("Q3_K_L") => 70,
            Some("IQ4_XS") | Some("IQ4_NL") => 65,
            Some(_) => 50,
            None => 40,
        }
    };

    let best_idx = recs
        .iter()
        .enumerate()
        .filter(|(_, r)| r.fit_level == "optimal")
        .max_by_key(|(_, r)| quant_rank(r.quantization.as_deref()))
        .or_else(|| {
            recs.iter()
                .enumerate()
                .filter(|(_, r)| r.fit_level == "good")
                .max_by_key(|(_, r)| quant_rank(r.quantization.as_deref()))
        })
        .map(|(idx, _)| idx);

    if let Some(idx) = best_idx {
        recs[idx].recommended = true;
    }

    recs
}

/// Compute summary statistics from a slice of telemetry records
pub fn summarize_telemetry(records: &[TelemetryRecord]) -> TelemetrySummary {
    if records.is_empty() {
        return TelemetrySummary::default();
    }

    let total_requests = records.len();
    let total_prompt_tokens: u64 = records.iter().map(|r| u64::from(r.prompt_tokens)).sum();
    let total_completion_tokens: u64 = records.iter().map(|r| u64::from(r.completion_tokens)).sum();
    let avg_tokens_per_second =
        records.iter().map(|r| r.tokens_per_second).sum::<f64>() / (total_requests as f64);
    let avg_latency_ms =
        records.iter().map(|r| r.duration_ms as f64).sum::<f64>() / (total_requests as f64);

    let mut recent = records.to_vec();
    recent.reverse();

    TelemetrySummary {
        total_requests,
        total_prompt_tokens,
        total_completion_tokens,
        avg_tokens_per_second,
        avg_latency_ms,
        recent,
    }
}

fn load_telemetry_from_disk(cache_dir: &std::path::Path) -> Vec<TelemetryRecord> {
    let path = cache_dir.join("telemetry.json");
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(records) = serde_json::from_str::<Vec<TelemetryRecord>>(&content) {
            return records;
        }
    }
    Vec::new()
}

fn save_telemetry_to_disk(cache_dir: &std::path::Path, records: &[TelemetryRecord]) {
    let path = cache_dir.join("telemetry.json");
    if let Ok(content) = serde_json::to_string_pretty(records) {
        let _ = std::fs::write(path, content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_q4_k_m_when_fitting_in_vram() {
        let files = vec![
            HubSibling {
                filename: "model.Q2_K.gguf".to_string(),
                size: Some(2_500_000_000),
            },
            HubSibling {
                filename: "model.Q4_K_M.gguf".to_string(),
                size: Some(4_500_000_000),
            },
            HubSibling {
                filename: "model.F16.gguf".to_string(),
                size: Some(15_000_000_000),
            },
        ];
        let devices = vec![GpuDevice::Cuda {
            index: 0,
            name: "RTX 4070".to_string(),
            vram_total: 12_000_000_000,
            vram_free: 10_000_000_000,
        }];

        let recs = recommend_gguf_files(&files, &devices, 16_000_000_000);
        assert_eq!(recs.len(), 3);
        let q4 = recs.iter().find(|r| r.filename.contains("Q4_K_M")).unwrap();
        assert_eq!(q4.fit_level, "optimal");
        assert!(q4.recommended);

        let f16 = recs.iter().find(|r| r.filename.contains("F16")).unwrap();
        assert_eq!(f16.fit_level, "too_large");
        assert!(!f16.recommended);
    }

    #[test]
    fn summarizes_telemetry_records_accurately() {
        let records = vec![
            TelemetryRecord {
                id: "1".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                model_id: "test-model".to_string(),
                backend: "candle".to_string(),
                device: "CPU".to_string(),
                prompt_tokens: 10,
                completion_tokens: 20,
                duration_ms: 1000,
                tokens_per_second: 20.0,
                time_to_first_token_ms: Some(100),
            },
            TelemetryRecord {
                id: "2".to_string(),
                timestamp: "2026-01-01T00:01:00Z".to_string(),
                model_id: "test-model".to_string(),
                backend: "candle".to_string(),
                device: "CPU".to_string(),
                prompt_tokens: 30,
                completion_tokens: 40,
                duration_ms: 1000,
                tokens_per_second: 40.0,
                time_to_first_token_ms: Some(80),
            },
        ];

        let summary = summarize_telemetry(&records);
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.total_prompt_tokens, 40);
        assert_eq!(summary.total_completion_tokens, 60);
        assert!((summary.avg_tokens_per_second - 30.0).abs() < 1e-6);
        assert_eq!(summary.recent[0].id, "2");
    }
}
