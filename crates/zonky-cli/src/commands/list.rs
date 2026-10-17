use console::style;

use zonky_core::hub::HubClient;
use zonky_core::ZonkyConfig;

pub fn run(config: &ZonkyConfig) -> anyhow::Result<()> {
    let hub = HubClient::new(config.cache_dir())?;
    let models = hub.list_local_models()?;

    if models.is_empty() {
        println!(
            "{} No local models found. Use {} to download one.",
            style("[INFO]").blue(),
            style("zonky pull <model-id>").bold()
        );
        return Ok(());
    }

    println!(
        "\n{} Local models ({}):\n",
        style("[FILES]").bold(),
        models.len()
    );

    // Table header
    println!(
        "  {:<40} {:<12} {:<12} {}",
        style("ID").bold().underlined(),
        style("SIZE").bold().underlined(),
        style("QUANT").bold().underlined(),
        style("REPO").bold().underlined(),
    );

    for model in &models {
        let size = bytesize::ByteSize(model.file_size).to_string();
        let quant = model.quantization.as_deref().unwrap_or("-");
        println!(
            "  {:<40} {:<12} {:<12} {}",
            model.id,
            size,
            quant,
            style(&model.repo_id).dim()
        );
    }

    println!();
    Ok(())
}
