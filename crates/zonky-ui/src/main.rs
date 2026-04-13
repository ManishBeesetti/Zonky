mod commands;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;
use zonky_core::model::ModelManager;
use zonky_core::ZonkyConfig;

use commands::ServerHandle;

#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("zonky=info")),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_devices,
            commands::get_system_info,
            commands::search_models,
            commands::list_local_models,
            commands::list_gguf_files,
            commands::pull_model,
            commands::delete_model,
            commands::load_model,
            commands::unload_model,
            commands::list_loaded_models,
            commands::chat_complete,
            commands::get_setup_status,
            commands::get_config,
            commands::start_server,
            commands::stop_server,
            commands::get_server_status,
        ])
        .setup(|app| {
            tracing::info!("Zonky desktop app starting");

            let config = ZonkyConfig::load().unwrap_or_default();
            let manager = ModelManager::new(config)
                .expect("Failed to initialize ModelManager");
            app.manage(Arc::new(manager));
            app.manage(Arc::new(Mutex::new(ServerHandle::new())));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Zonky");
}
