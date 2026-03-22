use anyhow::{Context, Result};
use colored::Colorize;
use self_update::cargo_crate_version;

const REPO_OWNER: &str = "nilutz";
const REPO_NAME: &str = "musictagger_rs";

/// Check if a new version is available
pub fn check_for_updates() -> Result<Option<String>> {
    println!("{}", "Checking for updates...".cyan());

    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .context("Failed to configure release list")?
        .fetch()
        .context("Failed to fetch releases from GitHub")?;

    if let Some(latest) = releases.first() {
        let current = cargo_crate_version!();
        let latest_version = latest.version.trim_start_matches('v');

        if latest_version != current {
            println!(
                "{} {} {} {}",
                "New version available:".green().bold(),
                latest_version.yellow(),
                "(current:".dimmed(),
                format!("{})", current).dimmed()
            );
            Ok(Some(latest_version.to_string()))
        } else {
            println!("{}", "You are running the latest version!".green());
            Ok(None)
        }
    } else {
        println!("{}", "No releases found.".yellow());
        Ok(None)
    }
}

/// Update the binary to the latest version
pub fn update() -> Result<()> {
    let current = cargo_crate_version!();
    println!("{} {}", "Current version:".cyan(), current.yellow());

    // Check if we have write permissions to the binary location
    if let Ok(exe_path) = std::env::current_exe() {
        if let Ok(metadata) = std::fs::metadata(&exe_path) {
            if metadata.permissions().readonly() {
                anyhow::bail!(
                    "Cannot update: binary is read-only.\n\
                     Binary location: {}\n\
                     Try running with elevated permissions (sudo) if installed system-wide.",
                    exe_path.display()
                );
            }
        }

        // On Unix, check if we can write to the parent directory
        #[cfg(unix)]
        if let Some(parent) = exe_path.parent() {
            use std::fs::OpenOptions;
            if OpenOptions::new().write(true).open(parent).is_err() {
                println!(
                    "{} Binary is installed in a system directory: {}",
                    "⚠".yellow(),
                    exe_path.display()
                );
                println!(
                    "{} You may need to run: {}",
                    "ℹ".cyan(),
                    format!("sudo {} --update", exe_path.display()).yellow()
                );
                println!();
            }
        }
    }

    println!("{}", "Checking for updates...".cyan());

    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("musictagger_rs")
        .show_download_progress(true)
        .current_version(current)
        .build()
        .context("Failed to configure updater")?
        .update()
        .context("Failed to update binary")?;

    println!(
        "\n{} {}",
        "Successfully updated to version".green().bold(),
        status.version().yellow()
    );
    println!(
        "{}",
        "Please restart the application to use the new version.".cyan()
    );

    Ok(())
}
