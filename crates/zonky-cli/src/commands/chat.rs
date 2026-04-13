use std::io::{self, BufRead, Write};

use console::style;

use zonky_core::types::{BackendChoice, DeviceChoice, GenerationRequest, Message, Role};
use zonky_core::{ModelManager, ZonkyConfig};

pub async fn run(config: &ZonkyConfig, model_id: &str) -> anyhow::Result<()> {
    let manager = ModelManager::new(config.clone())?;

    println!(
        "{} Loading model {}...",
        style("→").cyan(),
        style(model_id).bold()
    );

    manager
        .load_model(model_id, BackendChoice::Auto, DeviceChoice::Auto)
        .await?;

    println!(
        "\n{} Chat with {} (type 'exit' or Ctrl+C to quit)\n",
        style("💬").bold(),
        style(model_id).bold()
    );

    let mut messages: Vec<Message> = Vec::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("{} ", style("You:").green().bold());
        stdout.flush()?;

        let mut input = String::new();
        if stdin.lock().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == "exit" || input == "quit" {
            break;
        }

        // Special commands
        if input == "/clear" {
            messages.clear();
            println!("{} Chat history cleared\n", style("✓").green());
            continue;
        }

        messages.push(Message {
            role: Role::User,
            content: input.to_string(),
        });

        let request = GenerationRequest {
            model: model_id.to_string(),
            messages: messages.clone(),
            temperature: 0.7,
            top_p: 1.0,
            top_k: None,
            max_tokens: 2048,
            stream: false,
            repetition_penalty: 1.1,
            stop: None,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            seed: None,
        };

        print!("\n{} ", style("Assistant:").blue().bold());
        stdout.flush()?;

        match manager.generate(model_id, &request).await {
            Ok(response) => {
                if let Some(choice) = response.choices.first() {
                    println!("{}\n", choice.message.content);
                    messages.push(Message {
                        role: Role::Assistant,
                        content: choice.message.content.clone(),
                    });
                }
            }
            Err(e) => {
                println!("{} Error: {}\n", style("✗").red(), e);
            }
        }
    }

    println!("\n{} Goodbye!", style("👋").bold());
    manager.unload_model(model_id).await?;
    Ok(())
}
