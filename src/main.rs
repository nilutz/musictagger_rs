use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

mod manual_mode;
mod matcher;
mod musicbrainz;
mod tagger;

use matcher::match_files;
use musicbrainz::MusicBrainzClient;
use tagger::tag_files;

#[derive(Parser)]
#[command(name = "musictagger_rs")]
#[command(about = "Tag MP3 files with MusicBrainz metadata", long_about = None)]
struct Cli {
    /// Path to directory containing MP3 files
    #[arg(short, long)]
    path: PathBuf,

    /// MusicBrainz Release (Album) ID (required unless --manual is set)
    #[arg(short, long)]
    album_id: Option<String>,

    /// Manual tagging mode - enter metadata for each file interactively
    #[arg(short, long)]
    manual: bool,

    /// Dry run - show matches without writing tags
    #[arg(short, long)]
    dry_run: bool,

    /// Auto-confirm all matches without prompting
    #[arg(short = 'y', long)]
    yes: bool,

    /// Skip downloading cover art
    #[arg(long)]
    no_cover_art: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate that either album_id or manual mode is specified
    if cli.album_id.is_none() && !cli.manual {
        anyhow::bail!("Either --album-id or --manual must be specified");
    }
    if cli.album_id.is_some() && cli.manual {
        anyhow::bail!("Cannot use both --album-id and --manual at the same time");
    }

    println!("{}", "MusicBrainz MP3 Tagger".bright_cyan().bold());
    println!();

    // Validate and canonicalize path
    if !cli.path.exists() {
        anyhow::bail!("Path does not exist: {}", cli.path.display());
    }

    let path = cli.path.canonicalize().context("Failed to resolve path")?;

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    // Branch to manual mode if requested
    if cli.manual {
        return manual_mode::run(&path, cli.dry_run, cli.yes);
    }

    let album_id = cli.album_id.unwrap();

    // List all files in the directory
    println!("{}", "Files in directory:".bright_white());
    list_directory_contents(&path)?;
    println!();

    // Initialize MusicBrainz client
    println!(
        "{}",
        "Fetching album metadata from MusicBrainz...".bright_yellow()
    );
    let mb_client = MusicBrainzClient::new();
    let album = mb_client
        .get_release(&album_id)
        .await
        .context("Failed to fetch album from MusicBrainz")?;

    println!("{} {}", "✓".bright_green(), "Album found:".bright_white());
    println!(
        "  {} by {}",
        album.title.bright_cyan(),
        album.artist.bright_cyan()
    );
    if album.media_count > 1 {
        println!(
            "  {} discs, {} total tracks",
            album.media_count,
            album.tracks.len()
        );
    } else {
        println!("  {} tracks", album.tracks.len());
    }
    println!();

    // Fetch cover art
    let cover_art = if !cli.no_cover_art {
        println!("{}", "Fetching cover art...".bright_yellow());
        match mb_client.get_cover_art(&album_id).await {
            Ok(art) => {
                println!(
                    "{} Cover art downloaded ({:.1} KB)",
                    "✓".bright_green(),
                    art.len() as f64 / 1024.0
                );
                println!();
                Some(art)
            }
            Err(e) => {
                println!(
                    "{} {}: {}",
                    "⚠".bright_yellow(),
                    "Could not fetch cover art".bright_yellow(),
                    e
                );
                println!();
                None
            }
        }
    } else {
        println!("{}", "Skipping cover art download".bright_yellow());
        println!();
        None
    };

    // Find and match MP3 files
    println!("{}", "Matching files to tracks...".bright_yellow());
    let matches = match_files(&path, &album)?;

    if matches.is_empty() {
        println!(
            "{}",
            "Could not match any files to album tracks.".bright_red()
        );
        println!("This might happen if:");
        println!("  - The files don't belong to this album");
        println!("  - The file names are very different from track titles");
        println!("  - You specified the wrong MusicBrainz album ID");
        return Ok(());
    }

    println!();
    println!(
        "{} Matched {} of {} files",
        "✓".bright_green(),
        matches.len(),
        album.tracks.len()
    );
    println!();

    // In src/main.rs, update the final matches display section:

    // Display matches
    println!("{}", "Final matches:".bright_white().bold());
    println!();

    if album.media_count > 1 {
        let mut current_disc = 0;
        for (i, m) in matches.iter().enumerate() {
            if m.track.disc_number != current_disc {
                current_disc = m.track.disc_number;
                println!(
                    "\n{}",
                    format!("Disc {}:", current_disc).bright_cyan().bold()
                );
            }

            let confidence_color = if m.confidence > 0.7 {
                "bright green"
            } else if m.confidence > 0.4 {
                "bright yellow"
            } else {
                "bright red"
            };

            println!(
                "{}. {} (confidence: {})",
                (i + 1).to_string().bright_white(),
                m.file_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .bright_cyan(),
                format!("{:.0}%", m.confidence * 100.0).color(confidence_color)
            );
            println!(
                "   → Track {}: {} - {}",
                m.track.position,
                m.track.artist.bright_white(),
                m.track.title.bright_white()
            );
            println!();
        }
    } else {
        for (i, m) in matches.iter().enumerate() {
            let confidence_color = if m.confidence > 0.7 {
                "bright green"
            } else if m.confidence > 0.4 {
                "bright yellow"
            } else {
                "bright red"
            };

            println!(
                "{}. {} (confidence: {})",
                (i + 1).to_string().bright_white(),
                m.file_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .bright_cyan(),
                format!("{:.0}%", m.confidence * 100.0).color(confidence_color)
            );
            println!(
                "   → Track {}: {} - {}",
                m.track.position,
                m.track.artist.bright_white(),
                m.track.title.bright_white()
            );
            println!();
        }
    }

    if cli.dry_run {
        println!("{}", "Dry run - no files were modified.".bright_yellow());
        return Ok(());
    }

    // Confirm with user
    if !cli.yes {
        use dialoguer::Confirm;
        let confirmed = Confirm::new()
            .with_prompt("Do you want to apply these tags?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Aborted.".bright_yellow());
            return Ok(());
        }
    }

    // Apply tags
    println!();
    println!("{}", "Writing tags...".bright_yellow());
    tag_files(&matches, &album, cover_art)?;

    println!();
    println!(
        "{} {}",
        "✓".bright_green(),
        "Successfully tagged all files!".bright_green().bold()
    );

    Ok(())
}

fn list_directory_contents(path: &PathBuf) -> Result<()> {
    use std::fs;

    let mut entries: Vec<_> = fs::read_dir(path)
        .context("Failed to read directory")?
        .filter_map(|entry| entry.ok())
        .collect();

    // Sort by filename
    entries.sort_by_key(|entry| entry.file_name());

    if entries.is_empty() {
        println!("  {}", "(empty directory)".bright_black());
        return Ok(());
    }

    let mut mp3_count = 0;
    let mut other_count = 0;

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if path.is_file() {
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

            // Get file size
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let size_str = format_file_size(size);

            if extension.eq_ignore_ascii_case("mp3") {
                println!(
                    "  {} {} {}",
                    "♪".bright_cyan(),
                    file_name_str.bright_white(),
                    format!("({})", size_str).bright_black()
                );
                mp3_count += 1;
            } else {
                println!(
                    "  {} {} {}",
                    "·".bright_black(),
                    file_name_str.bright_black(),
                    format!("({})", size_str).bright_black()
                );
                other_count += 1;
            }
        } else if path.is_dir() {
            println!("  {} {}/", "📁".bright_blue(), file_name_str.bright_blue());
        }
    }

    println!();
    println!(
        "  {} {} MP3 file{}, {} other file{}",
        "Summary:".bright_white(),
        mp3_count,
        if mp3_count == 1 { "" } else { "s" },
        other_count,
        if other_count == 1 { "" } else { "s" }
    );

    Ok(())
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
