use console::style;

use zonky_core::hub::HubClient;
use zonky_core::ZonkyConfig;

pub async fn run(config: &ZonkyConfig, model_id: &str) -> anyhow::Result<()> {
    let hub = HubClient::new(config.cache_dir())?;

    // Check if it's a local model first
    let local_models = hub.list_local_models()?;
    if let Some(local) = local_models.iter().find(|m| m.id == model_id || m.repo_id == model_id) {
        println!("\n{} Local model info:\n", style("📋").bold());
        println!("  {} {}", style("ID:").bold(), local.id);
        println!("  {} {}", style("Repo:").bold(), local.repo_id);
        println!("  {} {}", style("File:").bold(), local.filename);
        println!("  {} {}", style("Size:").bold(), bytesize::ByteSize(local.file_size));
        println!("  {} {}", style("Path:").bold(), local.path.display());
        if let Some(ref q) = local.quantization {
            println!("  {} {}", style("Quantization:").bold(), q);
        }
        if let Some(ref a) = local.architecture {
            println!("  {} {}", style("Architecture:").bold(), a);
        }
        println!("  {} {}", style("Downloaded:").bold(), local.downloaded_at);
        println!();
        return Ok(());
    }

    // Try fetching from HuggingFace
    println!(
        "{} Fetching info for {}...\n",
        style("→").cyan(),
        style(model_id).bold()
    );

    let detail = hub.get_model_info(model_id).await?;

    println!("  {} {}", style("Model:").bold(), detail.model_id);
    if let Some(ref author) = detail.author {
        println!("  {} {}", style("Author:").bold(), author);
    }
    if let Some(ref pipeline) = detail.pipeline_tag {
        println!("  {} {}", style("Pipeline:").bold(), pipeline);
    }
    if let Some(downloads) = detail.downloads {
        println!("  {} {}", style("Downloads:").bold(), downloads);
    }
    if let Some(likes) = detail.likes {
        println!("  {} {}", style("Likes:").bold(), likes);
    }
    if !detail.tags.is_empty() {
        println!("  {} {}", style("Tags:").bold(), detail.tags.join(", "));
    }

    let gguf_files: Vec<_> = detail
        .siblings
        .iter()
        .filter(|s| s.filename.ends_with(".gguf"))
        .collect();

    if !gguf_files.is_empty() {
        println!("\n  {} GGUF files:", style("📦").bold());
        for f in &gguf_files {
            let size_str = f
                .size
                .map(|s| bytesize::ByteSize(s).to_string())
                .unwrap_or_else(|| "?".to_string());
            println!("    {} ({})", f.filename, size_str);
        }
    }

    println!();
    Ok(())
}
