use inquire::Select;

pub fn edit_api_keys() -> eyre::Result<()> {
    const API_KEY_NAMES: [&'static str; 5] = [
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "OPENROUTER_API_KEY",
        "SERPER_API_KEY",
        "JINA_API_KEY",
    ];

    loop {
        // choose a provider
        let Some(chosen_api_key_name) = Select::new(
            "Select an API key to change (ESC to abort):",
            API_KEY_NAMES.into(),
        )
        .prompt_skippable()?
        else {
            break;
        };

        let existing_value = std::env::var(&chosen_api_key_name).unwrap_or_default();
        let Some(new_value) = inquire::Text::new("Enter the new value (ESC to go back to menu):")
            .with_default(&existing_value)
            .prompt_skippable()?
        else {
            continue;
        };

        println!("Setting {} to {}", chosen_api_key_name, new_value);
    }

    Ok(())
}
