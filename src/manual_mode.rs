use anyhow::Result;
use colored::Colorize;
use dialoguer::Input;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ManualTrackInfo {
    pub file_path: PathBuf,
    pub title: String,
    pub artist: String,
    pub track_number: u32,
}

pub struct ManualAlbum {
    pub title: String,
    pub artist: String,
    pub tracks: Vec<ManualTrackInfo>,
    pub cover_art: Option<Vec<u8>>,
}

pub fn run(path: &Path, dry_run: bool, yes: bool) -> Result<()> {
    println!("{}", "Manual Tagging Mode".bright_cyan().bold());
    println!();

    // Collect MP3 files
    let files = collect_mp3_files(path)?;
    if files.is_empty() {
        anyhow::bail!("No MP3 files found in directory");
    }

    println!(
        "{} Found {} MP3 file(s)",
        "✓".bright_green(),
        files.len()
    );
    println!();

    // Try to get album info from existing tags of first file
    let first_file_tags = crate::tagger::read_existing_tags(&files[0]);

    let dir_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Album".to_string());

    let default_album = first_file_tags.album.unwrap_or(dir_name);
    let default_album_artist = first_file_tags.album_artist.unwrap_or_else(|| "Various Artists".to_string());

    let (album_title, album_artist, cover_art) = prompt_album_info(&default_album, &default_album_artist, path)?;
    println!();

    // Process each file
    println!("{}", "Enter metadata for each track:".bright_white().bold());
    println!("{}", "(Press Enter to accept suggested value)".bright_black());
    println!();

    let mut tracks = Vec::new();
    for (i, file_path) in files.iter().enumerate() {
        let filename = file_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        println!(
            "{} {}",
            format!("[{}/{}]", i + 1, files.len()).bright_black(),
            filename.bright_cyan()
        );

        // Read existing tags from file
        let existing_tags = crate::tagger::read_existing_tags(file_path);

        // Parse filename as fallback
        let (filename_artist, filename_title) = parse_filename(&filename);

        // Prefer existing tags, then filename parsing, then album artist
        let default_artist = existing_tags
            .artist
            .or(filename_artist)
            .unwrap_or_else(|| album_artist.clone());

        let default_title = existing_tags
            .title
            .unwrap_or(filename_title);

        let artist: String = Input::new()
            .with_prompt("  Artist")
            .default(default_artist)
            .interact_text()?;

        let title: String = Input::new()
            .with_prompt("  Title")
            .default(default_title)
            .interact_text()?;

        tracks.push(ManualTrackInfo {
            file_path: file_path.clone(),
            title,
            artist,
            track_number: (i + 1) as u32,
        });

        println!();
    }

    let album = ManualAlbum {
        title: album_title,
        artist: album_artist,
        tracks,
        cover_art,
    };

    // Show summary
    println!("{}", "Summary:".bright_white().bold());
    println!(
        "  Album: {} by {}",
        album.title.bright_cyan(),
        album.artist.bright_cyan()
    );
    if album.cover_art.is_some() {
        println!("  Cover art: {}", "Yes".bright_green());
    } else {
        println!("  Cover art: {}", "None".bright_yellow());
    }
    println!();
    for track in &album.tracks {
        println!(
            "  {}. {} - {}",
            track.track_number,
            track.artist.bright_white(),
            track.title.bright_white()
        );
    }
    println!();

    if dry_run {
        println!("{}", "Dry run - no files were modified.".bright_yellow());
        return Ok(());
    }

    // Confirm
    if !yes {
        use dialoguer::Confirm;
        let confirmed = Confirm::new()
            .with_prompt("Apply these tags?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Aborted.".bright_yellow());
            return Ok(());
        }
    }

    // Write tags
    println!();
    println!("{}", "Writing tags...".bright_yellow());
    crate::tagger::tag_files_manual(&album)?;

    println!();
    println!(
        "{} {}",
        "✓".bright_green(),
        "Successfully tagged all files!".bright_green().bold()
    );

    Ok(())
}

fn collect_mp3_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = WalkDir::new(path)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("mp3"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

fn prompt_album_info(default_album: &str, default_artist: &str, path: &Path) -> Result<(String, String, Option<Vec<u8>>)> {
    println!("{}", "Album Information:".bright_white().bold());

    let album_title: String = Input::new()
        .with_prompt("  Album Title")
        .default(default_album.to_string())
        .interact_text()?;

    let album_artist: String = Input::new()
        .with_prompt("  Album Artist")
        .default(default_artist.to_string())
        .interact_text()?;

    // Look for existing cover art in directory
    let default_cover = find_cover_art_in_dir(path);
    let default_cover_str = default_cover
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let cover_path: String = Input::new()
        .with_prompt("  Cover Art (path to image, or leave empty)")
        .default(default_cover_str)
        .allow_empty(true)
        .interact_text()?;

    let cover_art = if cover_path.is_empty() {
        None
    } else {
        let cover_path = Path::new(&cover_path);
        if cover_path.exists() {
            match std::fs::read(cover_path) {
                Ok(data) => {
                    println!(
                        "  {} Loaded cover art ({:.1} KB)",
                        "✓".bright_green(),
                        data.len() as f64 / 1024.0
                    );
                    Some(data)
                }
                Err(e) => {
                    println!(
                        "  {} Could not read cover art: {}",
                        "⚠".bright_yellow(),
                        e
                    );
                    None
                }
            }
        } else {
            println!(
                "  {} Cover art file not found: {}",
                "⚠".bright_yellow(),
                cover_path.display()
            );
            None
        }
    };

    Ok((album_title, album_artist, cover_art))
}

fn find_cover_art_in_dir(path: &Path) -> Option<PathBuf> {
    let image_extensions = ["jpg", "jpeg", "png", "webp"];
    let cover_names = ["cover", "folder", "album", "front", "artwork"];

    // First, look for files with common cover art names
    for name in &cover_names {
        for ext in &image_extensions {
            let filename = format!("{}.{}", name, ext);
            let cover_path = path.join(&filename);
            if cover_path.exists() {
                return Some(cover_path);
            }
            // Also try uppercase
            let filename_upper = format!("{}.{}", name.to_uppercase(), ext.to_uppercase());
            let cover_path = path.join(&filename_upper);
            if cover_path.exists() {
                return Some(cover_path);
            }
        }
    }

    // If no common name found, look for any image file
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if let Some(ext) = entry_path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if image_extensions.contains(&ext_lower.as_str()) {
                    return Some(entry_path);
                }
            }
        }
    }

    None
}

fn parse_filename(filename: &str) -> (Option<String>, String) {
    // Remove extension
    let name = filename
        .strip_suffix(".mp3")
        .or_else(|| filename.strip_suffix(".MP3"))
        .unwrap_or(filename);

    // Try to strip leading track numbers: "01 - ", "01. ", "1 - ", etc.
    let name = strip_track_number(name);

    // Try to split on " - " for "Artist - Title" pattern
    if let Some((artist, title)) = name.split_once(" - ") {
        let artist = artist.trim();
        let title = title.trim();

        // Check if artist looks like a track number (all digits)
        if artist.chars().all(|c| c.is_ascii_digit()) {
            return (None, title.to_string());
        }

        return (Some(artist.to_string()), title.to_string());
    }

    (None, name.trim().to_string())
}

fn strip_track_number(name: &str) -> &str {
    let name = name.trim();

    // Match patterns like "01 - ", "01. ", "1 - ", "1. ", "01 ", "1 "
    let chars: Vec<char> = name.chars().collect();

    // Find where the digits end
    let mut digit_end = 0;
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            digit_end = i + 1;
        } else {
            break;
        }
    }

    if digit_end == 0 {
        return name;
    }

    // Check what comes after the digits
    let rest = &name[digit_end..];
    let rest = rest.trim_start();

    // Strip separator if present
    if let Some(stripped) = rest.strip_prefix('-') {
        return stripped.trim_start();
    }
    if let Some(stripped) = rest.strip_prefix('.') {
        return stripped.trim_start();
    }

    // If there's no separator but rest starts with a letter, return rest
    if rest.starts_with(|c: char| c.is_alphabetic()) {
        return rest;
    }

    name
}
