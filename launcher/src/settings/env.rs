use inquire::Editor;
use std::fs;
use std::{io::Write, path::PathBuf};

/// Edit the environment file at the given path.
pub fn edit_environment_file(env_path: &PathBuf) -> eyre::Result<()> {
    let old_env_content = fs::read_to_string(env_path)?;

    let prompt = format!("Edit {} file:", env_path.display());
    let Some(new_env_content) = Editor::new(&prompt)
        .with_predefined_text(&old_env_content)
        .with_help_message("ESC to go back")
        .prompt_skippable()?
    else {
        return Ok(());
    };

    if old_env_content != new_env_content {
        let mut file = fs::File::create(env_path)?;
        file.write_all(new_env_content.as_bytes())?;

        println!("Environment file updated successfully.");
    } else {
        println!("No changes made to the file.");
    }

    Ok(())
}
