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
        // choose an API key name
        let Some(chosen_api_key_name) =
            Select::new("Select an API key to change:", API_KEY_NAMES.into())
                .with_help_message("↑↓ to move, enter to select, type to filter, ESC to go back")
                .prompt_skippable()?
        else {
            break;
        };

        // edit the API key
        let existing_value = std::env::var(&chosen_api_key_name).unwrap_or_default();
        let Some(new_value) = inquire::Text::new("Enter the new value:")
            .with_default(&existing_value)
            .with_help_message("ESC to go back")
            .prompt_skippable()?
        else {
            continue;
        };

        println!("Setting {} to {}", chosen_api_key_name, new_value);
    }

    Ok(())
}
