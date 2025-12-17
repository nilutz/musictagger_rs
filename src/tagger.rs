// src/tagger.rs
use anyhow::{Context, Result};
use id3::{frame, Tag, TagLike, Timestamp, Version};
use indicatif::{ProgressBar, ProgressStyle};

use crate::matcher::FileMatch;
use crate::musicbrainz::Album;

pub fn tag_files(matches: &[FileMatch], album: &Album, cover_art: Option<Vec<u8>>) -> Result<()> {
    let pb = ProgressBar::new(matches.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    for file_match in matches {
        pb.set_message(format!(
            "{}",
            file_match.file_path.file_name().unwrap().to_string_lossy()
        ));

        write_tags(
            &file_match.file_path,
            &file_match.track,
            album,
            cover_art.as_deref(),
        )
        .with_context(|| format!("Failed to write tags to {}", file_match.file_path.display()))?;

        pb.inc(1);
    }

    pb.finish_with_message("Complete");

    Ok(())
}

fn write_tags(
    file_path: &std::path::Path,
    track: &crate::musicbrainz::Track,
    album: &Album,
    cover_art: Option<&[u8]>,
) -> Result<()> {
    let mut tag = Tag::read_from_path(file_path).unwrap_or_else(|_| Tag::new());

    // Basic metadata
    tag.set_title(&track.title);
    tag.set_artist(&track.artist);
    tag.set_album(&album.title);
    tag.set_album_artist(&album.artist);
    tag.set_track(track.position);
    tag.set_total_tracks(album.total_tracks);

    // Disc number (only set if multi-disc release)
    if album.media_count > 1 {
        tag.set_disc(track.disc_number);
        tag.set_total_discs(album.media_count as u32);
    }

    // Year from date
    if let Some(date) = &album.date {
        if let Some(year_str) = date.split('-').next() {
            if let Ok(year) = year_str.parse::<i32>() {
                tag.set_year(year);
            }
        }

        if let Some(timestamp) = parse_date_to_timestamp(date) {
            tag.set_date_released(timestamp);
        }
    }

    // Add cover art
    if let Some(image_data) = cover_art {
        add_cover_art(&mut tag, image_data)?;
    }

    // MusicBrainz IDs
    add_txxx_frame(&mut tag, "MusicBrainz Album Id", &album.id);
    add_txxx_frame(&mut tag, "MusicBrainz Release Track Id", &track.id);
    add_txxx_frame(&mut tag, "MusicBrainz Recording Id", &track.recording_id);

    if let Some(artist_id) = &album.album_artist_id {
        add_txxx_frame(&mut tag, "MusicBrainz Album Artist Id", artist_id);
    }

    // Disc subtitle if present
    if let Some(disc_title) = &track.disc_title {
        tag.set_text("TSST", disc_title); // Set subtitle for disc
    }

    tag.write_to_path(file_path, Version::Id3v24)
        .context("Failed to write ID3 tag")?;

    Ok(())
}

fn add_cover_art(tag: &mut Tag, image_data: &[u8]) -> Result<()> {
    let mime_type = if image_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if image_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else {
        "image/jpeg"
    };

    let picture = frame::Picture {
        mime_type: mime_type.to_string(),
        picture_type: frame::PictureType::CoverFront,
        description: "Cover".to_string(),
        data: image_data.to_vec(),
    };

    tag.remove_picture_by_type(frame::PictureType::CoverFront);
    tag.add_frame(picture);

    Ok(())
}

fn parse_date_to_timestamp(date_str: &str) -> Option<Timestamp> {
    let parts: Vec<&str> = date_str.split('-').collect();

    match parts.len() {
        1 => {
            let year = parts[0].parse::<i32>().ok()?;
            Some(Timestamp {
                year,
                month: None,
                day: None,
                hour: None,
                minute: None,
                second: None,
            })
        }
        2 => {
            let year = parts[0].parse::<i32>().ok()?;
            let month = parts[1].parse::<u8>().ok()?;
            Some(Timestamp {
                year,
                month: Some(month),
                day: None,
                hour: None,
                minute: None,
                second: None,
            })
        }
        3 => {
            let year = parts[0].parse::<i32>().ok()?;
            let month = parts[1].parse::<u8>().ok()?;
            let day = parts[2].parse::<u8>().ok()?;
            Some(Timestamp {
                year,
                month: Some(month),
                day: Some(day),
                hour: None,
                minute: None,
                second: None,
            })
        }
        _ => None,
    }
}

fn add_txxx_frame(tag: &mut Tag, description: &str, value: &str) {
    let frame = frame::ExtendedText {
        description: description.to_string(),
        value: value.to_string(),
    };
    tag.add_frame(frame);
}
