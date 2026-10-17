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
        let cache = Self { cache_dir };
        let _ = cache.cleanup_partial_files();
        Ok(cache)
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

    /// Get the temporary partial file path used while downloading
    pub fn partial_model_path(&self, repo_id: &str, filename: &str) -> PathBuf {
        self.repo_dir(repo_id).join(format!("{filename}.part"))
    }

    /// Clean up any leftover `.part` files from interrupted or cancelled downloads
    pub fn cleanup_partial_files(&self) -> Result<usize> {
        let models_dir = self.cache_dir.join("models");
        if !models_dir.exists() {
            return Ok(0);
        }

        let mut removed = 0;
        if let Ok(repos) = std::fs::read_dir(&models_dir) {
            for repo_entry in repos.flatten() {
                let repo_path = repo_entry.path();
                if repo_path.is_dir() {
                    if let Ok(files) = std::fs::read_dir(&repo_path) {
                        for file_entry in files.flatten() {
                            let file_path = file_entry.path();
                            if file_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| n.ends_with(".part"))
                                .unwrap_or(false)
                            {
                                if std::fs::remove_file(&file_path).is_ok() {
                                    removed += 1;
                                }
                            }
                        }
                    }
                    let is_empty = std::fs::read_dir(&repo_path)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false);
                    if is_empty {
                        let _ = std::fs::remove_dir(&repo_path);
                    }
                }
            }
        }
        Ok(removed)
    }

    /// Clear all cached models and reset the registry
    pub fn clear_all(&self) -> Result<usize> {
        let registry = self.load_registry()?;
        let count = registry.models.len();

        let models_dir = self.cache_dir.join("models");
        if models_dir.exists() {
            std::fs::remove_dir_all(&models_dir)?;
        }

        self.save_registry(&Registry::default())?;
        info!(removed = count, "Cleared all models from cache");
        Ok(count)
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
            .find(|m| m.id == query || m.alias.as_deref() == Some(query) || m.repo_id == query)
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
pub fn generate_model_id(_repo_id: &str, filename: &str) -> String {
    // e.g. "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF" + "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
    // -> "tinyllama-1.1b-chat-v1.0-q4_k_m"
    let stem = filename
        .strip_suffix(".gguf")
        .or_else(|| filename.strip_suffix(".safetensors"))
        .unwrap_or(filename);

    stem.to_lowercase().replace('.', "-")
}

/// Extract quantization type from filename
pub fn extract_quantization(filename: &str) -> Option<String> {
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);

    // Look for common quantization patterns across '.' or '-' or '_' separators
    for part in stem.split(['.', '-']) {
        let quant_upper = part.to_uppercase();
        if quant_upper.starts_with("Q2_")
            || quant_upper.starts_with("Q3_")
            || quant_upper.starts_with("Q4_")
            || quant_upper.starts_with("Q5_")
            || quant_upper.starts_with("Q6_")
            || quant_upper.starts_with("Q8_")
            || quant_upper.starts_with("IQ1_")
            || quant_upper.starts_with("IQ2_")
            || quant_upper.starts_with("IQ3_")
            || quant_upper.starts_with("IQ4_")
            || quant_upper == "F16"
            || quant_upper == "BF16"
            || quant_upper == "F32"
        {
            return Some(quant_upper);
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache() -> (PathBuf, ModelCache) {
        let dir = std::env::temp_dir().join(format!("zonky_cache_test_{}", uuid::Uuid::new_v4()));
        let cache = ModelCache::new(dir.clone()).unwrap();
        (dir, cache)
    }

    #[test]
    fn registers_lists_deletes_and_clears_models() {
        let (dir, cache) = temp_cache();
        let model_path = cache.model_path(
            "Bartowski/Qwen2.5-7B-GGUF",
            "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        );
        std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
        std::fs::write(&model_path, b"dummy gguf bytes").unwrap();

        cache
            .register_model(
                "Bartowski/Qwen2.5-7B-GGUF",
                "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
                &model_path,
                16,
            )
            .unwrap();

        let models = cache.list_models().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].quantization.as_deref(), Some("Q4_K_M"));

        let id = models[0].id.clone();
        cache.set_alias(&id, "qwen").unwrap();
        assert!(cache.find_by_id_or_alias("qwen").unwrap().is_some());

        cache.delete_model(&id).unwrap();
        assert!(cache.list_models().unwrap().is_empty());
        assert!(!model_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleans_up_partial_download_files() {
        let (dir, cache) = temp_cache();
        let part_path = cache.partial_model_path("TheBloke/Model-GGUF", "model.Q4_K_M.gguf");
        std::fs::create_dir_all(part_path.parent().unwrap()).unwrap();
        std::fs::write(&part_path, b"partial data").unwrap();
        assert!(part_path.exists());

        let removed = cache.cleanup_partial_files().unwrap();
        assert_eq!(removed, 1);
        assert!(!part_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
