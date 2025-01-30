use inquire::Select;

pub fn edit_api_keys() -> eyre::Result<()> {
    let api_key_names = vec![
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "OPENROUTER_API_KEY",
        "SERPER_API_KEY",
        "JINA_API_KEY",
    ];
    // choose a provider
    let chosen_api_key_name =
        Select::new("Select an API key to change:", api_key_names).prompt()?;

    let existing_value = std::env::var(&chosen_api_key_name).unwrap_or_default();
    let new_value = inquire::Text::new("Enter the new value:")
        .with_default(&existing_value)
        .prompt()?;

    println!("Setting {} to {}", chosen_api_key_name, new_value);

    Ok(())
}
