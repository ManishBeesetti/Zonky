mod commands;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "zonky",
    about = "Zonky - High-performance local LLM inference engine",
    version,
    long_about = "Zonky is a blazing-fast, Rust-powered local LLM inference engine.\n\
                   Run large language models locally with GPU acceleration,\n\
                   an OpenAI-compatible API, and a beautiful terminal UI."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Backend to use for inference
    #[arg(long, global = true, default_value = "auto")]
    backend: String,

    /// Device to use (auto, cpu, cuda:0, metal)
    #[arg(long, global = true, default_value = "auto")]
    device: String,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Path to config file
    #[arg(long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the OpenAI-compatible API server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to bind to
        #[arg(long, default_value_t = 8080)]
        port: u16,
    },

    /// Download a model from HuggingFace Hub
    Pull {
        /// Model repo ID (e.g. TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF)
        model_id: String,
        /// Specific GGUF file to download (if omitted, lists available files)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Load a model and run a single prompt
    Run {
        /// Model ID or local path
        model_id: String,
        /// Prompt text
        #[arg(short, long)]
        prompt: Option<String>,
        /// Maximum tokens to generate
        #[arg(long, default_value_t = 512)]
        max_tokens: u32,
    },

    /// Interactive chat session with a model
    Chat {
        /// Model ID or local path
        model_id: String,
    },

    /// List locally downloaded models
    List,

    /// Search HuggingFace Hub for models
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// Delete a locally downloaded model
    Rm {
        /// Model ID to delete
        model_id: String,
    },

    /// Show detailed information about a model
    Info {
        /// Model ID (local or HuggingFace repo)
        model_id: String,
    },

    /// Launch the terminal UI
    Tui,

    /// Show system GPU/device information
    Devices,

    /// Detect GPU hardware and install required dependencies
    Setup {
        /// Automatically install missing dependencies
        #[arg(long)]
        install: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        "zonky=debug,tower_http=debug"
    } else {
        "zonky=info,zonky_core=info,zonky_server=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_target(false)
        .init();

    // Load config
    let mut config = if let Some(ref path) = cli.config {
        zonky_core::ZonkyConfig::load_from(&path.into())?
    } else {
        zonky_core::ZonkyConfig::load().unwrap_or_default()
    };

    match cli.command {
        Commands::Serve { host, port } => {
            config.server.host = host;
            config.server.port = port;
            commands::serve::run(config).await?;
        }
        Commands::Pull { model_id, file } => {
            commands::pull::run(&config, &model_id, file.as_deref()).await?;
        }
        Commands::Run {
            model_id,
            prompt,
            max_tokens,
        } => {
            commands::run::run(&config, &model_id, prompt.as_deref(), max_tokens).await?;
        }
        Commands::Chat { model_id } => {
            commands::chat::run(&config, &model_id).await?;
        }
        Commands::List => {
            commands::list::run(&config)?;
        }
        Commands::Search { query, limit } => {
            commands::search::run(&config, &query, limit).await?;
        }
        Commands::Rm { model_id } => {
            commands::rm::run(&config, &model_id)?;
        }
        Commands::Info { model_id } => {
            commands::info::run(&config, &model_id).await?;
        }
        Commands::Tui => {
            let tui_bin = std::env::current_exe()?
                .parent()
                .expect("no parent dir")
                .join("zonky-tui");
            if tui_bin.exists() {
                let status = std::process::Command::new(&tui_bin).status()?;
                std::process::exit(status.code().unwrap_or(1));
            } else {
                eprintln!("TUI binary not found. Build it with: cargo build -p zonky-tui");
                std::process::exit(1);
            }
        }
        Commands::Devices => {
            commands::devices::run();
        }
        Commands::Setup { install } => {
            commands::setup::run(install);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn help_text_has_no_mojibake() {
        let cmd = Cli::command();
        let about = cmd.get_about().map(|s| s.to_string()).unwrap_or_default();
        assert!(about.contains("Zonky -"));
        assert!(!about.contains('\u{00E2}'));
        assert!(!about.contains('\u{00F0}'));
        assert!(!about.contains('\u{00C2}'));
    }
}
