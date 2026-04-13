use zonky_core::{ModelManager, ZonkyConfig};

/// Shared application state
pub struct AppState {
    pub manager: ModelManager,
    pub config: ZonkyConfig,
}

impl AppState {
    pub fn new(manager: ModelManager, config: ZonkyConfig) -> Self {
        Self { manager, config }
    }
}
