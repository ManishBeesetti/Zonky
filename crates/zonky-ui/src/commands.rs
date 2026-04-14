use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use zonky_core::gpu;
use zonky_core::model::ModelManager;
use zonky_core::types::*;
use zonky_core::{HubClient, ZonkyConfig};

/// Handle for the background API server
pub struct ServerHandle {
    /// Abort handle to stop the server task
    abort_handle: Option<tokio::task::JoinHandle<()>>,
    pub running: bool,
    pub host: String,
    pub port: u16,
}

impl ServerHandle {
    pub fn new() -> Self {
        Self {
            abort_handle: None,
            running: false,
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub vendor: String,
    pub vram_total: u64,
    pub vram_free: u64,
    pub is_gpu: bool,
}

#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    pub devices: Vec<DeviceInfo>,
    pub total_memory: u64,
    pub available_memory: u64,
}

#[derive(Debug, Serialize)]
pub struct SetupStatusResponse {
    pub gpu_vendor: String,
    pub gpu_name: String,
    pub compute_ready: bool,
    pub needs_reboot: bool,
    pub deps: Vec<DepInfo>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct DepInfo {
    pub name: String,
    pub description: String,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
pub struct LocalModelInfo {
    pub id: String,
    pub repo_id: String,
    pub filename: String,
    pub size: u64,
    pub path: String,
    pub compatible_backends: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub model_id: String,
    pub author: Option<String>,
    pub downloads: u64,
    pub likes: u64,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct PullProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct GgufFileInfo {
    pub filename: String,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: u32,
}

fn build_embedded_server_state(
    manager: Arc<ModelManager>,
    config: ZonkyConfig,
) -> Arc<zonky_server::state::AppState> {
    Arc::new(zonky_server::state::AppState::new(manager, config))
}

fn get_hub_client() -> Result<HubClient, String> {
    let config = ZonkyConfig::load().unwrap_or_default();
    HubClient::new(config.cache_dir).map_err(|e| e.to_string())
}

// --- Device & System ---

#[tauri::command]
pub fn get_devices() -> Vec<DeviceInfo> {
    gpu::detect_devices()
        .into_iter()
        .map(|d| DeviceInfo {
            name: d.device_name(),
            vendor: d.vendor().to_string(),
            vram_total: d.vram_total(),
            vram_free: d.vram_free(),
            is_gpu: d.is_gpu(),
        })
        .collect()
}

#[tauri::command]
pub fn get_system_info() -> SystemInfoResponse {
    let devices: Vec<DeviceInfo> = gpu::detect_devices()
        .into_iter()
        .map(|d| DeviceInfo {
            name: d.device_name(),
            vendor: d.vendor().to_string(),
            vram_total: d.vram_total(),
            vram_free: d.vram_free(),
            is_gpu: d.is_gpu(),
        })
        .collect();

    // Read from /proc/meminfo
    let (total, available) = read_meminfo();

    SystemInfoResponse {
        devices,
        total_memory: total,
        available_memory: available,
    }
}

fn read_meminfo() -> (u64, u64) {
    let content = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0u64;
    let mut available = 0u64;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            total = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                * 1024;
        }
        if line.starts_with("MemAvailable:") {
            available = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                * 1024;
        }
    }
    (total, available)
}

#[tauri::command]
pub fn get_setup_status() -> SetupStatusResponse {
    let status = gpu::setup::check_setup();
    SetupStatusResponse {
        gpu_vendor: status.gpu_vendor,
        gpu_name: status.gpu_name,
        compute_ready: status.compute_ready,
        needs_reboot: status.needs_reboot,
        deps: status
            .deps
            .into_iter()
            .map(|d| DepInfo {
                name: d.name,
                description: d.description,
                installed: d.installed,
            })
            .collect(),
        summary: status.summary,
    }
}

#[tauri::command]
pub fn get_config() -> Result<serde_json::Value, String> {
    let config = ZonkyConfig::load().unwrap_or_default();
    serde_json::to_value(&config).map_err(|e| e.to_string())
}

// --- Model Hub ---

#[tauri::command]
pub async fn search_models(query: String, limit: usize) -> Result<Vec<SearchResult>, String> {
    let hub = get_hub_client()?;
    let results = hub
        .search_models(&query, limit)
        .await
        .map_err(|e| e.to_string())?;
    Ok(results
        .into_iter()
        .map(|m| SearchResult {
            model_id: m.model_id,
            author: m.author,
            downloads: m.downloads.unwrap_or(0),
            likes: m.likes.unwrap_or(0),
        })
        .collect())
}

#[tauri::command]
pub fn list_local_models(
    manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<LocalModelInfo>, String> {
    let hub = get_hub_client()?;
    let models = hub.list_local_models().map_err(|e| e.to_string())?;
    Ok(models
        .into_iter()
        .map(|m| {
            let compatible = manager.probe_compatible_backends(&m.path);
            LocalModelInfo {
                id: m.id,
                repo_id: m.repo_id,
                filename: m.filename,
                size: m.file_size,
                path: m.path.to_string_lossy().to_string(),
                compatible_backends: compatible,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn list_gguf_files(repo_id: String) -> Result<Vec<GgufFileInfo>, String> {
    let hub = get_hub_client()?;
    let files = hub
        .list_gguf_files(&repo_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(files
        .into_iter()
        .map(|f| GgufFileInfo {
            filename: f.filename,
            size: f.size,
        })
        .collect())
}

#[tauri::command]
pub async fn pull_model(
    repo_id: String,
    filename: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let hub = get_hub_client()?;
    let dl_id = format!("{repo_id}/{filename}");
    let dl_id_clone = dl_id.clone();
    let app_clone = app.clone();

    hub.download_model(&repo_id, &filename, move |progress| {
        let _ = app_clone.emit(
            "download-progress",
            serde_json::json!({
                "id": &dl_id_clone,
                "downloaded": progress.downloaded,
                "total": progress.total,
                "speed": progress.speed_bytes_per_sec,
            }),
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    // Also try to download tokenizer
    let _ = hub.download_tokenizer(&repo_id).await;

    Ok(format!("Downloaded {repo_id}/{filename}"))
}

#[tauri::command]
pub fn delete_model(model_id: String) -> Result<String, String> {
    let hub = get_hub_client()?;
    hub.delete_model(&model_id).map_err(|e| e.to_string())?;
    Ok(format!("Deleted {model_id}"))
}

// --- Inference ---

#[tauri::command]
pub async fn load_model(
    model_id: String,
    manager: State<'_, Arc<ModelManager>>,
) -> Result<String, String> {
    manager
        .load_model(&model_id, BackendChoice::Auto, DeviceChoice::Auto)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!("Model '{model_id}' loaded successfully"))
}

#[tauri::command]
pub async fn unload_model(
    model_id: String,
    manager: State<'_, Arc<ModelManager>>,
) -> Result<String, String> {
    manager
        .unload_model(&model_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!("Model '{model_id}' unloaded"))
}

#[tauri::command]
pub async fn list_loaded_models(
    manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<LoadedModelInfo>, String> {
    let models = manager.list_loaded_models().await;
    Ok(models
        .into_iter()
        .map(|m| LoadedModelInfo {
            id: m.id,
            backend: m.backend.unwrap_or_default(),
            vram_usage: m.vram_usage.unwrap_or(0),
            loaded: m.loaded,
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct LoadedModelInfo {
    pub id: String,
    pub backend: String,
    pub vram_usage: u64,
    pub loaded: bool,
}

// --- Server Control ---

#[derive(Debug, Serialize)]
pub struct ServerStatusResponse {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub url: String,
}

#[tauri::command]
pub async fn start_server(
    manager: State<'_, Arc<ModelManager>>,
    server: State<'_, Arc<Mutex<ServerHandle>>>,
) -> Result<ServerStatusResponse, String> {
    let mut handle = server.lock().await;
    if handle.running {
        return Ok(ServerStatusResponse {
            running: true,
            host: handle.host.clone(),
            port: handle.port,
            url: format!("http://{}:{}", handle.host, handle.port),
        });
    }

    let config = ZonkyConfig::load().unwrap_or_default();
    let host = config.server.host.clone();
    let port = config.server.port;
    let bind_addr = format!("{host}:{port}");

    // Reuse the same ModelManager used by desktop commands so loaded model state is shared.
    let state = build_embedded_server_state(Arc::clone(&manager), config);
    let app = zonky_server::build_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| format!("Failed to bind {bind_addr}: {e}"))?;

    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    handle.abort_handle = Some(task);
    handle.running = true;
    handle.host = "127.0.0.1".to_string();
    handle.port = port;

    tracing::info!(address = %bind_addr, "API server started from UI");

    Ok(ServerStatusResponse {
        running: true,
        host: handle.host.clone(),
        port: handle.port,
        url: format!("http://{}:{}", handle.host, handle.port),
    })
}

#[cfg(test)]
mod tests {
    use super::build_embedded_server_state;
    use std::sync::Arc;
    use zonky_core::{ModelManager, ZonkyConfig};

    #[test]
    fn embedded_server_state_reuses_shared_manager_arc() {
        let config = ZonkyConfig::default();
        let manager =
            Arc::new(ModelManager::new(config.clone()).expect("manager should initialize"));
        let state = build_embedded_server_state(Arc::clone(&manager), config);
        assert!(Arc::ptr_eq(&manager, &state.manager));
    }
}

#[tauri::command]
pub async fn stop_server(server: State<'_, Arc<Mutex<ServerHandle>>>) -> Result<String, String> {
    let mut handle = server.lock().await;
    if let Some(task) = handle.abort_handle.take() {
        task.abort();
    }
    handle.running = false;
    tracing::info!("API server stopped from UI");
    Ok("Server stopped".to_string())
}

#[tauri::command]
pub async fn get_server_status(
    server: State<'_, Arc<Mutex<ServerHandle>>>,
) -> Result<ServerStatusResponse, String> {
    let handle = server.lock().await;
    Ok(ServerStatusResponse {
        running: handle.running,
        host: handle.host.clone(),
        port: handle.port,
        url: format!("http://{}:{}", handle.host, handle.port),
    })
}

#[tauri::command]
pub async fn chat_complete(
    model: String,
    messages: Vec<Message>,
    manager: State<'_, Arc<ModelManager>>,
) -> Result<ChatResponse, String> {
    let request = GenerationRequest {
        model: model.clone(),
        messages,
        temperature: 0.7,
        top_p: 1.0,
        top_k: None,
        max_tokens: 2048,
        stream: false,
        repetition_penalty: 1.1,
        stop: None,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: None,
    };

    let response = manager
        .generate(&model, &request)
        .await
        .map_err(|e| e.to_string())?;

    let content = response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    let tokens_used = response.usage.total_tokens;

    Ok(ChatResponse {
        content,
        model: response.model,
        tokens_used,
    })
}
