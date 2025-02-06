use inquire::{validator::Validation, Text};

use crate::DriaEnv;

pub fn edit_ollama(dria_env: &mut DriaEnv) -> eyre::Result<()> {
    // asks for a change on both host and port
    if let Some(new_host) = Text::new("Enter host:")
        .with_help_message(&format!("ESC to change port instead"))
        .with_default(dria_env.get("OLLAMA_HOST").unwrap_or("http://localhost"))
        .with_validator(|host_str: &str| {
            if host_str.starts_with("http://") || host_str.starts_with("https://") {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(
                    "Host must start with http:// or https://.".into(),
                ))
            }
        })
        .prompt_skippable()?
    {
        dria_env.set("OLLAMA_HOST", new_host);
    }

    if let Some(new_port) = Text::new("Enter port:")
        .with_help_message(&format!("ESC to go back"))
        .with_default(dria_env.get("OLLAMA_PORT").unwrap_or("11434"))
        .with_validator(|port_str: &str| match u16::from_str_radix(port_str, 10) {
            Ok(_) => Ok(Validation::Valid),
            Err(_) => Ok(Validation::Invalid(
                "Port must be a valid 16-bit unsigned integer.".into(),
            )),
        })
        .prompt_skippable()?
    {
        dria_env.set("OLLAMA_PORT", new_port);
    };

    Ok(())
}
