// src/matcher.rs
use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::musicbrainz::{Album, Track};

#[derive(Debug)]
pub struct FileMatch {
    pub file_path: PathBuf,
    pub track: Track,
    pub confidence: f64,
}

pub fn match_files(path: &Path, album: &Album) -> Result<Vec<FileMatch>> {
    let mp3_files = find_mp3_files(path)?;
    
    if mp3_files.is_empty() {
        return Ok(Vec::new());
    }

    println!("Album tracks from MusicBrainz:");
    for track in &album.tracks {
        let duration = track.length
            .map(|ms| format!(" ({})", format_duration(ms)))
            .unwrap_or_default();
        println!("  {}. {}{}", track.position, track.title, duration);
    }
    println!();

    let matcher = SkimMatcherV2::default();
    
    // Try multiple passes with decreasing thresholds
    let thresholds = vec![80, 50, 30, 20, 10];
    let mut matches = Vec::new();
    let mut matched_tracks: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut matched_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for &threshold in &thresholds {
        println!("Matching pass (threshold: {})...", threshold);
        
        for file in &mp3_files {
            // Skip already matched files
            if matched_files.contains(file) {
                continue;
            }

            let file_name = file.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            // Get file duration if possible
            let file_duration = get_mp3_duration(file);
            
            if let Some((track, confidence, score)) = find_best_match_with_duration(
                &file, 
                &album, 
                &matcher,
                &matched_tracks,
                file_duration,
            ) {
                if score >= threshold {
                    let file_dur_str = file_duration
                        .map(|ms| format!(" [file: {}]", format_duration(ms)))
                        .unwrap_or_default();
                    let track_dur_str = track.length
                        .map(|ms| format!(" [track: {}]", format_duration(ms)))
                        .unwrap_or_default();
                    
                    println!("  ✓ {} -> Track {} - {} (score: {}, confidence: {}%){}{}",
                        file_name,
                        track.position,
                        track.title,
                        score,
                        (confidence * 100.0) as i32,
                        file_dur_str,
                        track_dur_str
                    );
                    
                    matched_tracks.insert(track.position);
                    matched_files.insert(file.clone());
                    
                    matches.push(FileMatch {
                        file_path: file.clone(),
                        track: track.clone(),
                        confidence,
                    });
                }
            }
        }
        
        // Stop if we've matched enough tracks
        if matches.len() >= album.tracks.len() {
            break;
        }
    }
    
    println!();
    
    if matches.len() < mp3_files.len() {
        println!("Unmatched files:");
        for file in &mp3_files {
            if !matched_files.contains(file) {
                let file_name = file.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let duration = get_mp3_duration(file)
                    .map(|ms| format!(" ({})", format_duration(ms)))
                    .unwrap_or_default();
                println!("  ✗ {}{}", file_name, duration);
            }
        }
        println!();
    }
    
    if matches.len() < album.tracks.len() {
        println!("Unmatched tracks:");
        for track in &album.tracks {
            if !matched_tracks.contains(&track.position) {
                let duration = track.length
                    .map(|ms| format!(" ({})", format_duration(ms)))
                    .unwrap_or_default();
                println!("  ✗ Track {} - {}{}", track.position, track.title, duration);
            }
        }
        println!();
    }

    // Sort by track position
    matches.sort_by_key(|m| m.track.position);

    Ok(matches)
}

fn find_mp3_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut mp3_files = Vec::new();

    if path.is_file() {
        if let Some(ext) = path.extension() {
            if ext.eq_ignore_ascii_case("mp3") {
                mp3_files.push(path.to_path_buf());
                return Ok(mp3_files);
            }
        }
        return Ok(mp3_files);
    }

    for entry in WalkDir::new(path)
        .min_depth(0)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path();
        
        if entry.file_type().is_file() {
            if let Some(ext) = entry_path.extension() {
                if ext.eq_ignore_ascii_case("mp3") {
                    mp3_files.push(entry_path.to_path_buf());
                }
            }
        }
    }

    Ok(mp3_files)
}

fn get_mp3_duration(file_path: &Path) -> Option<u32> {
    mp3_duration::from_path(file_path)
        .ok()
        .map(|duration| duration.as_millis() as u32)
}

fn find_best_match_with_duration<'a>(
    file_path: &Path,
    album: &'a Album,
    matcher: &SkimMatcherV2,
    matched_tracks: &std::collections::HashSet<u32>,
    file_duration: Option<u32>,
) -> Option<(&'a Track, f64, i64)> {
    let file_name = file_path
        .file_stem()?
        .to_string_lossy()
        .to_lowercase();

    let cleaned_name = clean_filename(&file_name);

    let mut best_match: Option<(&Track, i64)> = None;

    for track in &album.tracks {
        // Skip already matched tracks
        if matched_tracks.contains(&track.position) {
            continue;
        }

        let track_title_lower = track.title.to_lowercase();
        let track_artist_lower = track.artist.to_lowercase();
        let album_artist_lower = album.artist.to_lowercase();
        
        let mut max_score = 0i64;
        
        // Try matching with just the track title
        if let Some(score) = matcher.fuzzy_match(&cleaned_name, &track_title_lower) {
            max_score = max_score.max(score);
        }
        
        if let Some(score) = matcher.fuzzy_match(&file_name, &track_title_lower) {
            max_score = max_score.max(score);
        }
        
        // Try with track number prefix
        let with_track_num = format!("{} {}", track.position, track_title_lower);
        if let Some(score) = matcher.fuzzy_match(&cleaned_name, &with_track_num) {
            max_score = max_score.max(score);
        }
        
        let with_track_num_padded = format!("{:02} {}", track.position, track_title_lower);
        if let Some(score) = matcher.fuzzy_match(&cleaned_name, &with_track_num_padded) {
            max_score = max_score.max(score);
        }
        
        // Try with artist name (both track artist and album artist)
        let with_track_artist = format!("{} {}", track_artist_lower, track_title_lower);
        if let Some(score) = matcher.fuzzy_match(&cleaned_name, &with_track_artist) {
            max_score = max_score.max(score);
        }
        
        let with_album_artist = format!("{} {}", album_artist_lower, track_title_lower);
        if let Some(score) = matcher.fuzzy_match(&cleaned_name, &with_album_artist) {
            max_score = max_score.max(score);
        }
        
        // Substring matching bonus
        if cleaned_name.contains(&track_title_lower) {
            max_score = max_score.max(120);
        }
        if track_title_lower.contains(&cleaned_name) && cleaned_name.len() > 5 {
            max_score = max_score.max(100);
        }
        
        // Word matching score - significant words only
        let title_words: Vec<&str> = track_title_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3) // Only words longer than 3 characters
            .collect();
        
        if !title_words.is_empty() {
            let matching_words = title_words.iter()
                .filter(|word| cleaned_name.contains(*word))
                .count();
            
            let word_match_score = (matching_words as f64 / title_words.len() as f64 * 100.0) as i64;
            max_score = max_score.max(word_match_score);
        }
        
        // Duration matching bonus
        if let (Some(file_dur), Some(track_dur)) = (file_duration, track.length) {
            let duration_diff = (file_dur as i64 - track_dur as i64).abs();
            
            if duration_diff <= 5000 {
                // Within 5 seconds - excellent match
                let duration_bonus = 50 - (duration_diff / 100) as i64;
                max_score += duration_bonus.max(0);
            } else if duration_diff <= 10000 {
                // Within 10 seconds - good match
                max_score += 25;
            } else if duration_diff <= 30000 {
                // Within 30 seconds - decent match
                max_score += 10;
            }
        }

        if max_score > 0 {
            if let Some((_, best_score)) = best_match {
                if max_score > best_score {
                    best_match = Some((track, max_score));
                }
            } else {
                best_match = Some((track, max_score));
            }
        }
    }

    best_match.map(|(track, score)| {
        let confidence = (score as f64 / 150.0).min(1.0);
        (track, confidence, score)
    })
}

fn clean_filename(filename: &str) -> String {
    let mut cleaned = filename.to_string();
    
    // Remove text in square brackets (often YouTube IDs, catalog numbers, etc.)
    while let Some(bracket_start) = cleaned.find('[') {
        if let Some(bracket_end) = cleaned[bracket_start..].find(']') {
            let end_idx = bracket_start + bracket_end + 1;
            cleaned = format!("{}{}", &cleaned[..bracket_start], &cleaned[end_idx..]);
        } else {
            break;
        }
    }
    
    // Remove text in parentheses (often year, remaster info, featured artists, etc.)
    while let Some(paren_start) = cleaned.find('(') {
        if let Some(paren_end) = cleaned[paren_start..].find(')') {
            let end_idx = paren_start + paren_end + 1;
            cleaned = format!("{}{}", &cleaned[..paren_start], &cleaned[end_idx..]);
        } else {
            break;
        }
    }
    
    // Convert to lowercase
    cleaned = cleaned.to_lowercase();
    
    // Remove common separators and normalize
    // Replace hyphens, underscores, etc. with spaces
    cleaned = cleaned
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else {
                ' '
            }
        })
        .collect();
    
    // Collapse multiple spaces and trim
    cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    
    cleaned.trim().to_string()
}

fn format_duration(ms: u32) -> String {
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}