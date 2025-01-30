use inquire::{validator::Validation, Password, PasswordDisplayMode};

pub fn edit_wallet() -> eyre::Result<()> {
    let existing_key = std::env::var("DKN_WALLET_SECRET_KEY").unwrap_or_default();

    let mask = |s: &str| {
        const LEFT: usize = 2;
        const RIGHT: usize = 2;

        if s.len() <= LEFT + RIGHT {
            s.to_string()
        } else {
            format!(
                "{}{}{}",
                &s[..LEFT],
                "*".repeat(s.len() - LEFT - RIGHT),
                &s[s.len() - RIGHT..]
            )
        }
    };

    let validator = |secret_key: &str| {
        if secret_key.len() < 64 {
            Ok(Validation::Invalid(
                "Key must be at least 64 characters long.".into(),
            ))
        } else {
            Ok(Validation::Valid)
        }
    };

    let Some(new_key) = Password::new("Encryption key (ESC to abort):")
        .with_help_message(&format!(
            "Abort to keep the existing key: {}",
            mask(&existing_key)
        ))
        .with_validator(validator)
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        // .with_validator(|key| hex::decode(secret_env.trim_start_matches("0x")))
        .prompt_skippable()?
    else {
        return Ok(());
    };

    println!("New key: {:?}", mask(&new_key));

    Ok(())
}
