use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{Result, ZonkyError};
use crate::types::LocalModel;

const REGISTRY_FILE: &str = "registry.json";

/// Registry of locally cached models
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Registry {
    models: Vec<LocalModel>,
}

/// Manages the local model cache and registry
pub struct ModelCache {
    cache_dir: PathBuf,
}

impl ModelCache {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the directory for a specific repo's files
    pub fn repo_dir(&self, repo_id: &str) -> PathBuf {
        // Sanitize repo_id for filesystem: replace / with --
        let safe_id = repo_id.replace('/', "--");
        self.cache_dir.join("models").join(safe_id)
    }

    /// Get the full path where a model file would be stored
    pub fn model_path(&self, repo_id: &str, filename: &str) -> PathBuf {
        self.repo_dir(repo_id).join(filename)
    }

    /// Load the registry from disk
    fn load_registry(&self) -> Result<Registry> {
        let path = self.cache_dir.join(REGISTRY_FILE);
        if !path.exists() {
            return Ok(Registry::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let registry: Registry = serde_json::from_str(&content)
            .map_err(|e| ZonkyError::ConfigError(format!("Failed to parse registry: {e}")))?;

        Ok(registry)
    }

    /// Save the registry to disk
    fn save_registry(&self, registry: &Registry) -> Result<()> {
        let path = self.cache_dir.join(REGISTRY_FILE);
        let content = serde_json::to_string_pretty(registry)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Register a downloaded model in the local registry
    pub fn register_model(
        &self,
        repo_id: &str,
        filename: &str,
        path: &Path,
        file_size: u64,
    ) -> Result<()> {
        let mut registry = self.load_registry()?;

        // Generate a short ID from repo_id and filename
        let model_id = generate_model_id(repo_id, filename);

        // Remove existing entry if present
        registry.models.retain(|m| m.id != model_id);

        let quantization = extract_quantization(filename);

        let local_model = LocalModel {
            id: model_id.clone(),
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            path: path.to_path_buf(),
            file_size,
            alias: None,
            quantization,
            architecture: None,
            downloaded_at: chrono::Utc::now().to_rfc3339(),
        };

        registry.models.push(local_model);
        self.save_registry(&registry)?;

        info!(model_id = %model_id, "Model registered in cache");
        Ok(())
    }

    /// Find a model by repo_id and filename
    pub fn find_model(&self, repo_id: &str, filename: &str) -> Result<Option<LocalModel>> {
        let registry = self.load_registry()?;
        let model = registry
            .models
            .iter()
            .find(|m| m.repo_id == repo_id && m.filename == filename)
            .cloned();

        // Verify the file still exists on disk
        if let Some(ref m) = model {
            if !m.path.exists() {
                // File is gone, clean up registry
                debug!(model_id = %m.id, "Cached model file missing, cleaning registry");
                self.delete_model(&m.id)?;
                return Ok(None);
            }
        }

        Ok(model)
    }

    /// Find a model by its ID or alias
    pub fn find_by_id_or_alias(&self, query: &str) -> Result<Option<LocalModel>> {
        let registry = self.load_registry()?;
        let model = registry
            .models
            .iter()
            .find(|m| {
                m.id == query
                    || m.alias.as_deref() == Some(query)
                    || m.repo_id == query
            })
            .cloned();

        Ok(model)
    }

    /// List all registered models
    pub fn list_models(&self) -> Result<Vec<LocalModel>> {
        let registry = self.load_registry()?;
        Ok(registry.models)
    }

    /// Delete a model from cache and registry
    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        let mut registry = self.load_registry()?;

        if let Some(model) = registry.models.iter().find(|m| m.id == model_id) {
            // Delete the file
            if model.path.exists() {
                std::fs::remove_file(&model.path)?;
            }

            // Delete parent dir if empty
            if let Some(parent) = model.path.parent() {
                if parent.exists() {
                    let is_empty = std::fs::read_dir(parent)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false);
                    if is_empty {
                        let _ = std::fs::remove_dir(parent);
                    }
                }
            }
        }

        registry.models.retain(|m| m.id != model_id);
        self.save_registry(&registry)?;

        info!(model_id = %model_id, "Model deleted from cache");
        Ok(())
    }

    /// Set an alias for a model
    pub fn set_alias(&self, model_id: &str, alias: &str) -> Result<()> {
        let mut registry = self.load_registry()?;

        if let Some(model) = registry.models.iter_mut().find(|m| m.id == model_id) {
            model.alias = Some(alias.to_string());
            self.save_registry(&registry)?;
            Ok(())
        } else {
            Err(ZonkyError::ModelNotFound(model_id.to_string()))
        }
    }
}

/// Generate a human-readable model ID from repo_id and filename
fn generate_model_id(_repo_id: &str, filename: &str) -> String {
    // e.g. "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF" + "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
    // -> "tinyllama-1.1b-chat-v1.0-q4_k_m"
    let stem = filename
        .strip_suffix(".gguf")
        .or_else(|| filename.strip_suffix(".safetensors"))
        .unwrap_or(filename);

    stem.to_lowercase().replace('.', "-")
}

/// Extract quantization type from filename
fn extract_quantization(filename: &str) -> Option<String> {
    let stem = filename
        .strip_suffix(".gguf")
        .unwrap_or(filename);

    // Look for common quantization patterns
    let parts: Vec<&str> = stem.split('.').collect();
    if parts.len() >= 2 {
        let quant = parts.last().unwrap();
        let quant_upper = quant.to_uppercase();
        if quant_upper.starts_with('Q')
            || quant_upper.starts_with("IQ")
            || quant_upper.contains("K_M")
            || quant_upper.contains("K_S")
            || quant_upper.contains("K_L")
        {
            return Some(quant_upper);
        }
    }

    None
}
