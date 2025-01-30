use inquire::Editor;
use std::{io::Write, path::PathBuf};

/// Edit the environment file at the given path.
pub fn edit_environment_file(env_path: &PathBuf) -> eyre::Result<()> {
    let old_env_content = std::fs::read_to_string(env_path)?;
    let new_env_content = Editor::new(&format!("Edit environment at {}:", env_path.display()))
        .with_predefined_text(&old_env_content)
        .prompt()?;

    // Write the edited content back to the file
    if old_env_content != new_env_content {
        let mut file = std::fs::File::create(env_path)?;
        file.write_all(new_env_content.as_bytes())?;
        println!("Environment file updated successfully.");
    } else {
        println!("No changes made to the environment file.");
    }

    Ok(())
}
