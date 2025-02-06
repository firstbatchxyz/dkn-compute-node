use inquire::Select;
use self_update::{backends::github, update::Release};

struct DriaRelease(Release);

impl std::fmt::Display for DriaRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name)
    }
}

pub async fn change_version() -> eyre::Result<()> {
    // https://github.com/jaemk/self_update/issues/44
    let releases = tokio::task::spawn_blocking(move || {
        let mut rel_builder = github::ReleaseList::configure();

        rel_builder
            .repo_owner("firstbatchxyz")
            .repo_name("dkn-compute-node")
            .build()
            .unwrap() // TODO:!!!
            .fetch()
            .unwrap() // TODO:!!!
            .into_iter()
            .map(|r| DriaRelease(r))
            .collect::<Vec<_>>()
    })
    .await?;

    // .iter().filter(|r| r.version.starts_with);

    let Some(chosen_release) = Select::new("Select a version:", releases)
        .with_help_message("↑↓ to move, enter to select, type to filter, ESC to go back")
        .prompt_skippable()?
    else {
        return Ok(());
    };

    println!("Chosen version: {}", chosen_release);

    Ok(())
}
