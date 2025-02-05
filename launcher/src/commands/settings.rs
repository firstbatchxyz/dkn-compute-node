use inquire::{Confirm, Select};
use std::path::PathBuf;

use crate::{settings::*, DriaEnv};

/// Starts the interactive settings editor for the given environment.
pub fn change_settings(env_path: &PathBuf) -> eyre::Result<()> {
    // an environment object is created from the existing environment variables
    let mut dria_env = DriaEnv::new();

    loop {
        // prompt the user for which setting to change
        let Some(choice) = Select::new(
            &format!("Choose settings (for {})", env_path.display()),
            Settings::all(),
        )
        .with_help_message("↑↓ to move, enter to select, type to filter, ESC to quit")
        .prompt_skippable()?
        else {
            if dria_env.is_changed() {
                // continue the loop if user returns `false` from confirmation
                if let Some(false) =
                    Confirm::new("You have unsaved changes, are you sure you want to quit (y/n)?")
                        .with_help_message("You will LOSE all unsaved changes if you confirm.")
                        .prompt_skippable()?
                {
                    continue;
                }
            }

            println!("Exiting...");
            break;
        };

        match choice {
            Settings::Wallet => {
                crate::settings::edit_wallet(&mut dria_env)?;
            }
            Settings::Port => {
                crate::settings::edit_port(&mut dria_env)?;
            }
            Settings::Models => {
                crate::settings::edit_models(&mut dria_env)?;
            }
            Settings::Ollama => {
                crate::settings::edit_ollama(&mut dria_env)?;
            }
            Settings::ApiKeys => {
                crate::settings::edit_api_keys(&mut dria_env)?;
            }
            Settings::SaveExit => {
                if dria_env.is_changed() {
                    dria_env.save_to_file(env_path)?;
                } else {
                    println!("No changes made.");
                }
                break;
            }
        }
    }

    Ok(())
}
