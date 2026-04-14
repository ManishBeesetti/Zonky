use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::error::{Result, ZonkyError};
use crate::gpu::{self, GpuDevice};
use crate::hub::HubClient;
use crate::inference::candle_backend::CandleBackend;
#[cfg(feature = "llamacpp")]
use crate::inference::llamacpp_backend::LlamaCppBackend;
use crate::inference::{InferenceBackend, ModelHandle};
use crate::types::*;

/// A loaded model with its metadata and backend reference
struct LoadedModel {
    handle: ModelHandle,
    info: ModelInfo,
    backend_name: String,
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
    config: super::config::ZonkyConfig,
    /// Available GPU devices
    devices: Vec<GpuDevice>,
}

impl ModelManager {
    pub fn new(config: super::config::ZonkyConfig) -> Result<Self> {
        let mut backends: HashMap<String, Box<dyn InferenceBackend>> = HashMap::new();

        // Register both inference backends
        backends.insert("candle".to_string(), Box::new(CandleBackend::new()));
        #[cfg(feature = "llamacpp")]
        backends.insert("llamacpp".to_string(), Box::new(LlamaCppBackend::new()));

        let hub = HubClient::new(config.cache_dir())?;
        let devices = gpu::detect_devices();

        info!(
            backends = ?backends.keys().collect::<Vec<_>>(),
            devices = ?devices.iter().map(|d| d.device_name()).collect::<Vec<_>>(),
            "ModelManager initialized"
        );

        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            backends,
            hub,
            config,
            devices,
        })
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

        // Resolve model path
        let model_path = self.resolve_model_path(model_id)?;

        // Select device
        let device = match device_choice {
            DeviceChoice::Auto => self.best_device(),
            DeviceChoice::Cpu => GpuDevice::Cpu,
            DeviceChoice::Cuda(idx) => {
                self.devices
                    .iter()
                    .find(|d| matches!(d, GpuDevice::Cuda { index, .. } if *index == idx))
                    .cloned()
                    .ok_or_else(|| ZonkyError::GpuError(format!("CUDA device {idx} not found")))?
            }
            DeviceChoice::Metal => {
                self.devices
                    .iter()
                    .find(|d| matches!(d, GpuDevice::Metal { .. }))
                    .cloned()
                    .ok_or_else(|| ZonkyError::GpuError("Metal device not found".to_string()))?
            }
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
                let used: u64 = self.models.read().await.values().map(|m| m.vram_usage).sum();
                let free = available.saturating_sub(used);

                if estimated_vram > free {
                    if self.config.auto_evict {
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
                        last_used: Instant::now(),
                        vram_usage: estimated_vram,
                    };

                    self.models.write().await.insert(model_id.to_string(), loaded);
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
        let models = self.models.read().await;
        let loaded = models
            .get(model_id)
            .ok_or_else(|| ZonkyError::ModelNotFound(model_id.to_string()))?;

        let backend = self
            .backends
            .get(&loaded.backend_name)
            .ok_or_else(|| ZonkyError::BackendError(loaded.backend_name.clone()))?;

        // Update last_used (need write lock)
        drop(models);
        if let Some(model) = self.models.write().await.get_mut(model_id) {
            model.last_used = Instant::now();
        }

        let models = self.models.read().await;
        let loaded = models.get(model_id).unwrap();
        backend.generate(&loaded.handle, request).await
    }

    /// Generate a streaming response
    pub async fn generate_stream(
        &self,
        model_id: &str,
        request: &GenerationRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let models = self.models.read().await;
        let loaded = models
            .get(model_id)
            .ok_or_else(|| ZonkyError::ModelNotFound(model_id.to_string()))?;

        let (tx, rx) = mpsc::channel(64);
        let handle = loaded.handle.clone();
        let request = request.clone();

        // Update last_used
        drop(models);
        if let Some(model) = self.models.write().await.get_mut(model_id) {
            model.last_used = Instant::now();
        }

        // Get backend ref for spawning
        let backend_name = self
            .models
            .read()
            .await
            .get(model_id)
            .map(|m| m.backend_name.clone())
            .ok_or_else(|| ZonkyError::ModelNotFound(model_id.to_string()))?;

        let backend2 = self
            .backends
            .get(&backend_name)
            .ok_or_else(|| ZonkyError::BackendError(backend_name))?;

        backend2.generate_stream(&handle, &request, tx).await?;

        Ok(rx)
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
        let has_rocm = self.devices.iter().any(|d| matches!(d, GpuDevice::Rocm { .. }));
        let has_cuda = self.devices.iter().any(|d| matches!(d, GpuDevice::Cuda { .. }));
        let has_metal = self.devices.iter().any(|d| matches!(d, GpuDevice::Metal { .. }));

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
