//! Helper functions to check the latest version of the node.
//!
//! The URL: <https://api.github.com/repos/firstbatchxyz/dkn-compute-node/releases/latest> always holds the final tag.
use self_update::backends::github;

pub fn check_releases() -> eyre::Result<()> {
    let mut rel_builder = github::ReleaseList::configure();

    rel_builder.repo_owner("firstbatchxyz");
    rel_builder.repo_name("dkn-compute-node");

    let releases = rel_builder.build()?.fetch()?;
    println!("found releases:");
    println!("{:#?}\n", releases);

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_releases() {
        super::check_releases().unwrap();
    }
}
