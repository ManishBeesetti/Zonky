use std::sync::Arc;

use zonky_core::{ModelManager, ZonkyConfig};

/// Shared application state
pub struct AppState {
    pub manager: Arc<ModelManager>,
    pub config: ZonkyConfig,
}

impl AppState {
    pub fn new(manager: Arc<ModelManager>, config: ZonkyConfig) -> Self {
        Self { manager, config }
    }
}
