use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use zonky_core::hub::HubClient;
use zonky_core::ZonkyConfig;

pub async fn run(config: &ZonkyConfig, model_id: &str, file: Option<&str>) -> anyhow::Result<()> {
    let hub = HubClient::new(config.cache_dir())?;

    // If no specific file requested, list available GGUF files
    let filename = if let Some(f) = file {
        f.to_string()
    } else {
        println!(
            "{} Fetching available files for {}...",
            style("→").cyan(),
            style(model_id).bold()
        );

        let files = hub.list_gguf_files(model_id).await?;

        if files.is_empty() {
            println!(
                "{} No GGUF files found in {}",
                style("✗").red(),
                model_id
            );
            return Ok(());
        }

        println!("\n{} Available GGUF files:\n", style("📦").bold());
        for (i, f) in files.iter().enumerate() {
            let size_str = f
                .size
                .map(|s| bytesize::ByteSize(s).to_string())
                .unwrap_or_else(|| "unknown size".to_string());
            println!("  {} {} ({})", style(format!("[{i}]")).dim(), f.filename, size_str);
        }

        println!();

        // Use dialoguer to select
        let selection = dialoguer::Select::new()
            .with_prompt("Select a file to download")
            .items(
                &files
                    .iter()
                    .map(|f| &f.filename as &str)
                    .collect::<Vec<_>>(),
            )
            .default(0)
            .interact()?;

        files[selection].filename.clone()
    };

    println!(
        "\n{} Downloading {}/{}\n",
        style("⬇").cyan(),
        style(model_id).bold(),
        style(&filename).bold()
    );

    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );

    let pb_clone = pb.clone();
    let path = hub
        .download_model(model_id, &filename, move |progress| {
            if let Some(total) = progress.total {
                pb_clone.set_length(total);
            }
            pb_clone.set_position(progress.downloaded);
        })
        .await?;

    pb.finish_with_message("done");

    // Also try to download the tokenizer
    println!(
        "\n{} Downloading tokenizer...",
        style("⬇").cyan()
    );
    match hub.download_tokenizer(model_id).await {
        Ok(_) => println!("{} Tokenizer downloaded", style("✓").green()),
        Err(_) => println!("{} Tokenizer not available (will use fallback)", style("⚠").yellow()),
    }

    println!(
        "\n{} Model saved to {}\n",
        style("✓").green(),
        style(path.display()).bold()
    );

    Ok(())
}
