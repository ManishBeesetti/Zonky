use zonky_core::{ModelManager, ZonkyConfig};

pub async fn run(config: ZonkyConfig) -> anyhow::Result<()> {
    let manager = ModelManager::new(config.clone())?;
    zonky_server::serve(manager, config).await
}
