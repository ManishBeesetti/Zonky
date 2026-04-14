use console::style;

use zonky_core::types::{BackendChoice, DeviceChoice, GenerationRequest, Message, Role};
use zonky_core::{ModelManager, ZonkyConfig};

pub async fn run(
    config: &ZonkyConfig,
    model_id: &str,
    prompt: Option<&str>,
    max_tokens: u32,
) -> anyhow::Result<()> {
    let manager = ModelManager::new(config.clone())?;

    println!(
        "{} Loading model {}...",
        style("[->]").cyan(),
        style(model_id).bold()
    );

    manager
        .load_model(model_id, BackendChoice::Auto, DeviceChoice::Auto)
        .await?;

    let prompt_text = if let Some(p) = prompt {
        p.to_string()
    } else {
        // Read from stdin
        println!("{}", style("Enter your prompt (Ctrl+D to finish):").dim());
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)?;
        input
    };

    println!("\n{} Generating...\n", style("[RUN]").yellow());

    let request = GenerationRequest {
        model: model_id.to_string(),
        messages: vec![Message {
            role: Role::User,
            content: prompt_text,
        }],
        temperature: 0.7,
        top_p: 1.0,
        top_k: None,
        max_tokens,
        stream: false,
        repetition_penalty: 1.1,
        stop: None,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: None,
    };

    let response = manager.generate(model_id, &request).await?;

    if let Some(choice) = response.choices.first() {
        println!("{}", choice.message.content);
    }

    println!(
        "\n{}",
        style(format!(
            "({} prompt + {} completion = {} total tokens)",
            response.usage.prompt_tokens,
            response.usage.completion_tokens,
            response.usage.total_tokens
        ))
        .dim()
    );

    manager.unload_model(model_id).await?;
    Ok(())
}
