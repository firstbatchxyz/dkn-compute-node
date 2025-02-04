use inquire::Select;
use std::path::PathBuf;

use crate::settings::Settings;

pub fn change_settings(env_path: &PathBuf) -> eyre::Result<()> {
    loop {
        let Some(choice) = Select::new(
            &format!("Choose settings (for {})", env_path.display()),
            Settings::all(),
        )
        .with_help_message("↑↓ to move, enter to select, type to filter, ESC to quit")
        .prompt_skippable()?
        else {
            println!("Exiting...");
            break;
        };

        match choice {
            Settings::Wallet => {
                crate::settings::edit_wallet()?;
            }
            Settings::Models => {
                crate::settings::edit_models()?;
            }
            Settings::ApiKeys => {
                crate::settings::edit_api_keys()?;
            }
            Settings::SaveExit => {
                println!("Saving to {}", env_path.display());
                // TODO: !!!
                break;
            }
        }
    }

    Ok(())
}
