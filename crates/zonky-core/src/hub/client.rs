use std::path::{Path, PathBuf};

use reqwest::Client;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::error::{Result, ZonkyError};
use crate::hub::cache::ModelCache;
use crate::types::{DownloadProgress, HubModelDetail, HubModelInfo, HubSibling, LocalModel};

const HF_API_BASE: &str = "https://huggingface.co/api";

/// Client for interacting with the HuggingFace Hub
#[derive(Clone)]
pub struct HubClient {
    http: Client,
    download_http: Client,
    cache: ModelCache,
}

/// Returns true if a HuggingFace Hub repository appears to be a text/chat/vision LLM
/// rather than a diffusion/image/video/audio generation model packaged as GGUF.
pub fn is_llm_compatible_repo(model: &HubModelInfo) -> bool {
    const NON_LLM_PIPELINES: &[&str] = &[
        "text-to-image",
        "image-to-image",
        "image-to-video",
        "text-to-video",
        "video-to-video",
        "audio-to-audio",
        "text-to-speech",
        "text-to-audio",
        "automatic-speech-recognition",
        "unconditional-image-generation",
        "image-segmentation",
        "object-detection",
        "depth-estimation",
        "mask-generation",
    ];

    if let Some(ref pipeline) = model.pipeline_tag {
        let p = pipeline.to_lowercase();
        if NON_LLM_PIPELINES.contains(&p.as_str()) {
            return false;
        }
    }

    if let Some(ref lib) = model.library_name {
        let l = lib.to_lowercase();
        if matches!(l.as_str(), "diffusers" | "comfyui") {
            return false;
        }
    }

    for tag in &model.tags {
        let t = tag.to_lowercase();
        if matches!(
            t.as_str(),
            "diffusers"
                | "stable-diffusion"
                | "stable-diffusion-xl"
                | "flux"
                | "text-to-image"
                | "image-to-image"
                | "image-to-video"
                | "text-to-video"
                | "comfyui"
        ) {
            return false;
        }
    }

    let id_lower = model.model_id.to_lowercase();
    const NON_LLM_ID_PATTERNS: &[&str] = &[
        "qwen-image",
        "stable-diffusion",
        "sdxl",
        "flux.",
        "flux-",
        "wan2.1",
        "hunyuan-video",
        "hunyuanvideo",
        "mochi-1",
        "cogvideo",
        "ltx-video",
        "sd3.",
        "sd35",
        "auraflow",
        "pixart",
        "controlnet",
    ];
    if NON_LLM_ID_PATTERNS.iter().any(|pat| id_lower.contains(pat)) {
        return false;
    }

    true
}

impl HubClient {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        // Pick up HF auth token from environment or ~/.cache/huggingface/token
        let hf_token = Self::resolve_hf_token();

        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(ref token) = hf_token {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
                headers.insert(reqwest::header::AUTHORIZATION, val);
            }
        }

        let http = Client::builder()
            .user_agent("zonky/0.1.0")
            .default_headers(headers.clone())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ZonkyError::HubError(format!("Failed to create HTTP client: {e}")))?;

        // Separate client for large file downloads — no total timeout
        let download_http = Client::builder()
            .user_agent("zonky/0.1.0")
            .default_headers(headers)
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ZonkyError::HubError(format!("Failed to create download client: {e}")))?;

        let cache = ModelCache::new(cache_dir)?;

        if hf_token.is_some() {
            debug!("Using HuggingFace auth token");
        }

        Ok(Self {
            http,
            download_http,
            cache,
        })
    }

    /// Return a new `HubClient` sharing the existing HTTP clients but pointing at `cache_dir`
    pub fn with_cache_dir(&self, cache_dir: PathBuf) -> Result<Self> {
        let cache = ModelCache::new(cache_dir)?;
        Ok(Self {
            http: self.http.clone(),
            download_http: self.download_http.clone(),
            cache,
        })
    }

    /// Resolve HF token from env vars or cached token file
    fn resolve_hf_token() -> Option<String> {
        // 1. HF_TOKEN env var (official)
        if let Ok(token) = std::env::var("HF_TOKEN") {
            if !token.is_empty() {
                return Some(token);
            }
        }
        // 2. HUGGING_FACE_HUB_TOKEN (legacy)
        if let Ok(token) = std::env::var("HUGGING_FACE_HUB_TOKEN") {
            if !token.is_empty() {
                return Some(token);
            }
        }
        // 3. Cached token from `huggingface-cli login`
        if let Some(base) = directories::BaseDirs::new() {
            let token_path = base.cache_dir().join("huggingface").join("token");
            if let Ok(token) = std::fs::read_to_string(&token_path) {
                let token = token.trim().to_string();
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }
        None
    }

    /// Create with default cache directory (~/.cache/zonky)
    pub fn with_default_cache() -> Result<Self> {
        let cache_dir = default_cache_dir();
        Self::new(cache_dir)
    }

    /// Search for LLM-compatible GGUF models on HuggingFace Hub
    pub async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<HubModelInfo>> {
        #[derive(Serialize)]
        struct SearchParams<'a> {
            search: &'a str,
            limit: usize,
            sort: &'a str,
            direction: i8,
            filter: &'a str,
        }

        let fetch_limit = limit.saturating_mul(2).max(20);
        let url = format!("{HF_API_BASE}/models");
        let params = SearchParams {
            search: query,
            limit: fetch_limit,
            sort: "downloads",
            direction: -1,
            filter: "gguf",
        };

        debug!(query = %query, limit, fetch_limit, "Searching HuggingFace Hub");

        let response = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| ZonkyError::HubError(format!("Search request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(ZonkyError::HubError(format!(
                "HF API returned status {}",
                response.status()
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| ZonkyError::HubError(format!("Failed to read response: {e:?}")))?;

        let raw_models: Vec<HubModelInfo> = serde_json::from_str(&text).map_err(|e| {
            ZonkyError::HubError(format!(
                "Failed to parse search results: {e} near byte {}",
                e.column()
            ))
        })?;

        let models: Vec<HubModelInfo> = raw_models
            .into_iter()
            .filter(is_llm_compatible_repo)
            .take(limit)
            .collect();

        info!(count = models.len(), query = %query, "Search returned LLM-compatible models");
        Ok(models)
    }

    /// Get detailed information about a specific model
    pub async fn get_model_info(&self, repo_id: &str) -> Result<HubModelDetail> {
        let url = format!("{HF_API_BASE}/models/{repo_id}");

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ZonkyError::HubError(format!("Model info request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(ZonkyError::HubError(format!(
                "HF API returned status {} for {repo_id}",
                response.status()
            )));
        }

        let text = response.text().await.map_err(|e| {
            ZonkyError::HubError(format!("Failed to read model info response: {e}"))
        })?;

        let detail: HubModelDetail = serde_json::from_str(&text)
            .map_err(|e| ZonkyError::HubError(format!("Failed to parse model info: {e}")))?;

        Ok(detail)
    }

    /// List available GGUF files for a model repo using the tree API (includes file sizes)
    pub async fn list_gguf_files(&self, repo_id: &str) -> Result<Vec<HubSibling>> {
        let url = format!("{HF_API_BASE}/models/{repo_id}/tree/main");

        debug!(url = %url, "Listing repo files via tree API");

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ZonkyError::HubError(format!("Tree request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ZonkyError::HubError(format!(
                    "HF API returned {status} for {repo_id}: repository is gated or private. Set HF_TOKEN to authenticate."
                )));
            }
            return Err(ZonkyError::HubError(format!(
                "HF API returned status {status} for {repo_id}"
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| ZonkyError::HubError(format!("Failed to read tree response: {e}")))?;

        #[derive(serde::Deserialize)]
        struct TreeEntry {
            #[serde(default)]
            path: String,
            #[serde(default)]
            size: Option<u64>,
            #[serde(default, rename = "type")]
            entry_type: Option<String>,
        }

        let entries: Vec<TreeEntry> = serde_json::from_str(&text)
            .map_err(|e| ZonkyError::HubError(format!("Failed to parse tree response: {e}")))?;

        let gguf_files: Vec<HubSibling> = entries
            .into_iter()
            .filter(|e| e.entry_type.as_deref() == Some("file") && e.path.ends_with(".gguf"))
            .map(|e| HubSibling {
                filename: e.path,
                size: e.size,
            })
            .collect();

        info!(count = gguf_files.len(), repo_id = %repo_id, "Found GGUF files");
        Ok(gguf_files)
    }

    /// Download a model file from HuggingFace Hub with progress tracking
    pub async fn download_model<F>(
        &self,
        repo_id: &str,
        filename: &str,
        progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: Fn(DownloadProgress) + Send + 'static,
    {
        self.download_model_cancellable(repo_id, filename, None, progress_callback)
            .await
    }

    /// Download a model file from HuggingFace Hub with progress tracking and optional cancellation token.
    /// Downloads to a temporary `.part` file and atomically renames on completion; cleans up `.part` on error/cancel.
    pub async fn download_model_cancellable<F>(
        &self,
        repo_id: &str,
        filename: &str,
        cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: Fn(DownloadProgress) + Send + 'static,
    {
        // Check if already cached
        if let Some(local) = self.cache.find_model(repo_id, filename)? {
            info!(repo_id = %repo_id, filename = %filename, "Model already cached");
            return Ok(local.path);
        }

        let url = format!("https://huggingface.co/{repo_id}/resolve/main/{filename}");

        info!(url = %url, "Downloading model");

        let response = self
            .download_http
            .get(&url)
            .send()
            .await
            .map_err(|e| ZonkyError::DownloadError(format!("Download request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ZonkyError::DownloadError(format!(
                    "Download failed ({status}): {repo_id} requires authentication. Set HF_TOKEN."
                )));
            }
            return Err(ZonkyError::DownloadError(format!(
                "Download failed with status {status}"
            )));
        }

        let total_size = response.content_length();
        let dest_path = self.cache.model_path(repo_id, filename);
        let part_path = self.cache.partial_model_path(repo_id, filename);

        // Ensure parent directory exists
        if let Some(parent) = part_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Download to .part file with progress tracking and cancellation support
        let mut file = tokio::fs::File::create(&part_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let start = std::time::Instant::now();
        let mut last_report = start
            .checked_sub(std::time::Duration::from_secs(1))
            .unwrap_or(start);

        use futures::StreamExt;
        let download_result: Result<()> = async {
            while let Some(chunk) = stream.next().await {
                if let Some(ref token) = cancel_token {
                    if token.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(ZonkyError::Cancelled(format!(
                            "Download of {repo_id}/{filename} cancelled"
                        )));
                    }
                }

                let chunk =
                    chunk.map_err(|e| ZonkyError::DownloadError(format!("Stream error: {e}")))?;
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;

                let is_complete = total_size.map(|t| downloaded >= t).unwrap_or(false);
                if is_complete || last_report.elapsed() >= std::time::Duration::from_millis(100) {
                    let elapsed = start.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (downloaded as f64 / elapsed) as u64
                    } else {
                        0
                    };

                    progress_callback(DownloadProgress {
                        downloaded,
                        total: total_size,
                        speed_bytes_per_sec: speed,
                    });
                    last_report = std::time::Instant::now();
                }
            }
            file.flush().await?;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (downloaded as f64 / elapsed) as u64
            } else {
                0
            };
            progress_callback(DownloadProgress {
                downloaded,
                total: total_size.or(Some(downloaded)),
                speed_bytes_per_sec: speed,
            });
            Ok(())
        }
        .await;

        drop(file);

        if let Err(err) = download_result {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(err);
        }

        // Atomically rename .part to destination
        if let Err(err) = tokio::fs::rename(&part_path, &dest_path).await {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(ZonkyError::IoError(err));
        }

        // Register in cache
        let file_size = downloaded;
        self.cache
            .register_model(repo_id, filename, &dest_path, file_size)?;

        info!(
            repo_id = %repo_id,
            filename = %filename,
            size = file_size,
            "Download complete"
        );

        Ok(dest_path)
    }

    /// Download the tokenizer for a model repo.
    /// GGUF repos typically don't include tokenizer.json, so we try multiple sources:
    /// 1. The GGUF repo itself
    /// 2. The base model repo (strip "-GGUF" suffix, try original author via HF API)
    pub async fn download_tokenizer(&self, repo_id: &str) -> Result<PathBuf> {
        let dest_dir = self.cache.repo_dir(repo_id);
        tokio::fs::create_dir_all(&dest_dir).await?;
        let dest_path = dest_dir.join("tokenizer.json");

        // Build candidate repo IDs to try
        let mut candidates = vec![repo_id.to_string()];

        // If this is a GGUF repo, try to find the source model repo
        let repo_name = repo_id.split('/').nth(1).unwrap_or("");
        if repo_name.ends_with("-GGUF") {
            let base_name = repo_name.strip_suffix("-GGUF").unwrap();
            let author = repo_id.split('/').next().unwrap_or("");

            // Try same author without -GGUF
            candidates.push(format!("{author}/{base_name}"));

            // Try to resolve via HF API model card (base_model field)
            if let Ok(detail) = self.get_model_info(repo_id).await {
                for tag in &detail.tags {
                    // Match "base_model:author/name" but skip "base_model:quantized:..."
                    if let Some(rest) = tag.strip_prefix("base_model:") {
                        if !rest.starts_with("quantized:") && rest.contains('/') {
                            let base_repo = rest.trim().to_string();
                            // Also try unsloth mirror (often ungated)
                            if let Some(model_name) = base_repo.split('/').nth(1) {
                                candidates.push(format!("unsloth/{model_name}"));
                            }
                            candidates.push(base_repo);
                        }
                    }
                }
            }
        }

        // Try each candidate
        for candidate in &candidates {
            let url = format!("https://huggingface.co/{candidate}/resolve/main/tokenizer.json");
            debug!(repo = %candidate, "Trying tokenizer source");

            let response = match self.http.get(&url).send().await {
                Ok(r) if r.status().is_success() => r,
                _ => continue,
            };

            let bytes = match response.bytes().await {
                Ok(b) => b,
                Err(_) => continue,
            };

            tokio::fs::write(&dest_path, &bytes).await?;
            info!(repo_id = %repo_id, source = %candidate, "Tokenizer downloaded");
            return Ok(dest_path);
        }

        Err(ZonkyError::DownloadError(format!(
            "tokenizer.json not found in any of: {}",
            candidates.join(", ")
        )))
    }

    /// List all locally cached models
    pub fn list_local_models(&self) -> Result<Vec<LocalModel>> {
        self.cache.list_models()
    }

    /// Delete a locally cached model
    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        self.cache.delete_model(model_id)
    }

    /// Clear all locally cached models
    pub fn clear_cache(&self) -> Result<usize> {
        self.cache.clear_all()
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &Path {
        self.cache.cache_dir()
    }
}

/// Default cache directory
pub fn default_cache_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.cache_dir().join("zonky"))
        .unwrap_or_else(|| PathBuf::from(".cache/zonky"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_hub_model(model_id: &str, pipeline_tag: Option<&str>, tags: &[&str]) -> HubModelInfo {
        HubModelInfo {
            model_id: model_id.to_string(),
            author: None,
            sha: None,
            downloads: Some(100),
            likes: Some(10),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            pipeline_tag: pipeline_tag.map(|s| s.to_string()),
            library_name: None,
            last_modified: None,
        }
    }

    #[test]
    fn filters_out_diffusion_and_non_llm_repos() {
        let qwen_image = mock_hub_model(
            "unsloth/Qwen-Image-Edit-2511-GGUF",
            Some("image-to-image"),
            &["gguf", "diffusers"],
        );
        assert!(!is_llm_compatible_repo(&qwen_image));

        let flux = mock_hub_model("city96/FLUX.1-dev-gguf", Some("text-to-image"), &["gguf"]);
        assert!(!is_llm_compatible_repo(&flux));

        let qwen_llm = mock_hub_model(
            "Qwen/Qwen2.5-7B-Instruct-GGUF",
            Some("text-generation"),
            &["gguf", "conversational"],
        );
        assert!(is_llm_compatible_repo(&qwen_llm));
    }
}
