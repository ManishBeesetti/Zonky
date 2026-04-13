use console::style;

use zonky_core::hub::HubClient;
use zonky_core::ZonkyConfig;

pub async fn run(config: &ZonkyConfig, query: &str, limit: usize) -> anyhow::Result<()> {
    let hub = HubClient::new(config.cache_dir())?;

    println!(
        "{} Searching HuggingFace for \"{}\"...\n",
        style("🔍").bold(),
        style(query).bold()
    );

    let models = hub.search_models(query, limit).await?;

    if models.is_empty() {
        println!("{} No models found for \"{}\"", style("ℹ").blue(), query);
        return Ok(());
    }

    for model in &models {
        let downloads = model
            .downloads
            .map(|d| format!("⬇ {d}"))
            .unwrap_or_default();
        let likes = model
            .likes
            .map(|l| format!("♥ {l}"))
            .unwrap_or_default();

        println!(
            "  {} {} {}",
            style(&model.model_id).bold().cyan(),
            style(&downloads).dim(),
            style(&likes).dim(),
        );

        if !model.tags.is_empty() {
            let tags: Vec<&str> = model.tags.iter().take(5).map(|s| s.as_str()).collect();
            println!("    {}", style(tags.join(", ")).dim());
        }
        println!();
    }

    println!(
        "  {} Use {} to download a model",
        style("💡").bold(),
        style("zonky pull <model-id>").bold()
    );

    Ok(())
}
