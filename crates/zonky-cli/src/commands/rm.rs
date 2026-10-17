use console::style;

use zonky_core::hub::HubClient;
use zonky_core::ZonkyConfig;

pub fn run(config: &ZonkyConfig, model_id: &str) -> anyhow::Result<()> {
    let hub = HubClient::new(config.cache_dir())?;

    hub.delete_model(model_id)?;

    println!(
        "{} Deleted model {}",
        style("[OK]").green(),
        style(model_id).bold()
    );

    Ok(())
}
