use inquire::{validator::Validation, Password, PasswordDisplayMode};

use crate::DriaEnv;

pub fn edit_wallet(dria_env: &mut DriaEnv) -> eyre::Result<()> {
    // masks a string "abcdefgh" to something like "ab****gh"
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

    // validates the secret key to be 64 characters hexadecimal, with or without 0x prefix
    let validator = |secret_key: &str| {
        if secret_key.trim_start_matches("0x").len() != 64 {
            Ok(Validation::Invalid(
                "Key must be exactly 64 characters hexadecimal, with or without 0x prefix.".into(),
            ))
        } else {
            Ok(Validation::Valid)
        }
    };

    let Some(new_key) = Password::new("Enter wallet secret key:")
        .with_help_message(&format!(
            "ESC to go back and keep using {}",
            mask(
                &dria_env
                    .get("DKN_WALLET_SECRET_KEY")
                    .unwrap_or(&"".to_string())
            )
        ))
        .with_validator(validator)
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt_skippable()?
    else {
        return Ok(());
    };

    println!("New key: {:?}", mask(&new_key));
    dria_env.set("DKN_WALLET_SECRET_KEY", new_key);

    Ok(())
}
