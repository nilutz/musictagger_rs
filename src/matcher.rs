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

    if album.media_count > 1 {
        let mut current_disc = 0;
        for track in &album.tracks {
            if track.disc_number != current_disc {
                current_disc = track.disc_number;
                let disc_info = if let Some(title) = &track.disc_title {
                    format!(" - {}", title)
                } else {
                    String::new()
                };
                println!("\n  Disc {}{}:", current_disc, disc_info);
            }
            let duration = track
                .length
                .map(|ms| format!(" ({})", format_duration(ms)))
                .unwrap_or_default();
            println!("    {}. {}{}", track.position, track.title, duration);
        }
    } else {
        for track in &album.tracks {
            let duration = track
                .length
                .map(|ms| format!(" ({})", format_duration(ms)))
                .unwrap_or_default();
            println!("  {}. {}{}", track.position, track.title, duration);
        }
    }
    println!();

    let matcher = SkimMatcherV2::default();

    // PHASE 1: Score all possible file-to-track combinations
    println!("Computing all possible matches...");

    #[derive(Debug, Clone)]
    struct PossibleMatch {
        file_idx: usize,
        track_idx: usize,
        score: i64,
        confidence: f64,
    }

    let mut all_possible_matches: Vec<PossibleMatch> = Vec::new();

    for (file_idx, file) in mp3_files.iter().enumerate() {
        let file_duration = get_mp3_duration(file);

        for (track_idx, track) in album.tracks.iter().enumerate() {
            if let Some((_, confidence, score)) =
                score_match(file, track, &matcher, file_duration, &album.artist)
            {
                all_possible_matches.push(PossibleMatch {
                    file_idx,
                    track_idx,
                    score,
                    confidence,
                });
            }
        }
    }

    // PHASE 2: Sort by score (highest first)
    all_possible_matches.sort_by(|a, b| b.score.cmp(&a.score));

    // PHASE 3: Greedily assign matches, preventing conflicts
    let mut matched_files: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut matched_tracks: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut final_matches: Vec<FileMatch> = Vec::new();

    println!("\nAssigning matches (highest confidence first)...");

    for possible in all_possible_matches {
        // Skip if either file or track already matched
        if matched_files.contains(&possible.file_idx)
            || matched_tracks.contains(&possible.track_idx)
        {
            continue;
        }

        let file = &mp3_files[possible.file_idx];
        let track = &album.tracks[possible.track_idx];

        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let file_duration = get_mp3_duration(file);
        let file_dur_str = file_duration
            .map(|ms| format!(" [file: {}]", format_duration(ms)))
            .unwrap_or_default();
        let track_dur_str = track
            .length
            .map(|ms| format!(" [track: {}]", format_duration(ms)))
            .unwrap_or_default();

        if album.media_count > 1 {
            println!(
                "  ✓ {} -> Disc {} Track {} - {} (score: {}, confidence: {}%){}{}",
                file_name,
                track.disc_number,
                track.position,
                track.title,
                possible.score,
                (possible.confidence * 100.0) as i32,
                file_dur_str,
                track_dur_str
            );
        } else {
            println!(
                "  ✓ {} -> Track {} - {} (score: {}, confidence: {}%){}{}",
                file_name,
                track.position,
                track.title,
                possible.score,
                (possible.confidence * 100.0) as i32,
                file_dur_str,
                track_dur_str
            );
        }

        matched_files.insert(possible.file_idx);
        matched_tracks.insert(possible.track_idx);

        final_matches.push(FileMatch {
            file_path: file.clone(),
            track: track.clone(),
            confidence: possible.confidence,
        });
    }

    println!();

    // Report unmatched files
    if matched_files.len() < mp3_files.len() {
        println!("Unmatched files:");
        for (idx, file) in mp3_files.iter().enumerate() {
            if !matched_files.contains(&idx) {
                let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let duration = get_mp3_duration(file)
                    .map(|ms| format!(" ({})", format_duration(ms)))
                    .unwrap_or_default();
                println!("  ✗ {}{}", file_name, duration);
            }
        }
        println!();
    }

    // Report unmatched tracks
    if matched_tracks.len() < album.tracks.len() {
        println!("Unmatched tracks:");
        for (idx, track) in album.tracks.iter().enumerate() {
            if !matched_tracks.contains(&idx) {
                let duration = track
                    .length
                    .map(|ms| format!(" ({})", format_duration(ms)))
                    .unwrap_or_default();

                if album.media_count > 1 {
                    println!(
                        "  ✗ Disc {} Track {} - {}{}",
                        track.disc_number, track.position, track.title, duration
                    );
                } else {
                    println!("  ✗ Track {} - {}{}", track.position, track.title, duration);
                }
            }
        }
        println!();
    }

    // Sort final matches by disc number, then track position
    final_matches.sort_by_key(|m| (m.track.disc_number, m.track.position));

    // Filter out very low confidence matches
    let filtered_matches: Vec<FileMatch> = final_matches
        .into_iter()
        .filter(|m| {
            if m.confidence < 0.15 {
                println!(
                    "⚠ Skipping very low confidence match: {} -> {} ({}%)",
                    m.file_path.file_name().unwrap().to_string_lossy(),
                    m.track.title,
                    (m.confidence * 100.0) as i32
                );
                false
            } else {
                true
            }
        })
        .collect();

    Ok(filtered_matches)
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

/// Score a single file-track pairing
fn score_match<'a>(
    file_path: &Path,
    track: &'a Track,
    matcher: &SkimMatcherV2,
    file_duration: Option<u32>,
    album_artist: &str,
) -> Option<(&'a Track, f64, i64)> {
    let file_name = file_path.file_stem()?.to_string_lossy().to_lowercase();

    let (base_name, file_qualifiers) = extract_qualifiers(&file_name);
    let cleaned_name = clean_filename(&file_name);

    let track_title_lower = track.title.to_lowercase();
    let track_artist_lower = track.artist.to_lowercase();
    let album_artist_lower = album_artist.to_lowercase();

    let (track_base, track_qualifiers) = extract_qualifiers(&track_title_lower);

    // Calculate base similarity score
    let mut base_score = 0i64;

    if let Some(score) = matcher.fuzzy_match(&base_name, &track_base) {
        base_score = base_score.max(score);
    }

    if let Some(score) = matcher.fuzzy_match(&file_name, &track_title_lower) {
        base_score = base_score.max(score);
    }

    if let Some(score) = matcher.fuzzy_match(&cleaned_name, &track_base) {
        base_score = base_score.max(score);
    }

    let with_track_num = format!("{} {}", track.position, track_base);
    if let Some(score) = matcher.fuzzy_match(&base_name, &with_track_num) {
        base_score = base_score.max(score);
    }

    let with_track_artist = format!("{} {}", track_artist_lower, track_base);
    if let Some(score) = matcher.fuzzy_match(&base_name, &with_track_artist) {
        base_score = base_score.max(score);
    }

    let with_album_artist = format!("{} {}", album_artist_lower, track_base);
    if let Some(score) = matcher.fuzzy_match(&base_name, &with_album_artist) {
        base_score = base_score.max(score);
    }

    // Word matching for better accuracy
    let title_words: Vec<&str> = track_base
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .collect();

    if !title_words.is_empty() {
        let matching_words = title_words
            .iter()
            .filter(|word| base_name.contains(*word))
            .count();
        let word_ratio = matching_words as f64 / title_words.len() as f64;
        let word_score = (word_ratio * 100.0) as i64;
        base_score = base_score.max(word_score);
    }

    // Require minimum base similarity
    if base_score < 30 {
        return None;
    }

    // Qualifier matching - CRITICAL for distinguishing versions
    let has_file_qualifiers = !file_qualifiers.is_empty();
    let has_track_qualifiers = !track_qualifiers.is_empty();

    let qualifier_score = match (has_file_qualifiers, has_track_qualifiers) {
        (true, true) => {
            // Both have qualifiers - check if they match
            let matching_qualifiers: Vec<_> = file_qualifiers
                .iter()
                .filter(|fq| {
                    track_qualifiers.iter().any(|tq| {
                        let fq_words: Vec<&str> = fq.split_whitespace().collect();
                        let tq_words: Vec<&str> = tq.split_whitespace().collect();

                        fq_words.iter().any(|fw| {
                            tq_words.iter().any(|tw| {
                                tw.contains(fw)
                                    || fw.contains(tw)
                                    || (fw.len() > 4
                                        && tw.len() > 4
                                        && (fw.starts_with(&tw[..tw.len().min(5)])
                                            || tw.starts_with(&fw[..fw.len().min(5)])))
                            })
                        })
                    })
                })
                .collect();

            if matching_qualifiers.is_empty() {
                // Both have qualifiers but they don't match - wrong!
                -1000
            } else {
                // Qualifiers match - big bonus
                100 * matching_qualifiers.len() as i64
            }
        }
        (true, false) => {
            // File has qualifiers but track doesn't
            -200
        }
        (false, true) => {
            // Track has qualifiers but file doesn't
            -200
        }
        (false, false) => {
            // Neither has qualifiers - small bonus
            20
        }
    };

    // Duration matching bonus
    let duration_score = if let (Some(file_dur), Some(track_dur)) = (file_duration, track.length) {
        let duration_diff = (file_dur as i64 - track_dur as i64).abs();

        if duration_diff <= 3000 {
            80 // Within 3 seconds - excellent
        } else if duration_diff <= 5000 {
            50 // Within 5 seconds - very good
        } else if duration_diff <= 10000 {
            25 // Within 10 seconds - good
        } else if duration_diff <= 30000 {
            10 // Within 30 seconds - acceptable
        } else {
            0
        }
    } else {
        0
    };

    let total_score = base_score + qualifier_score + duration_score;

    if total_score > 0 {
        let confidence = (total_score as f64 / 200.0).min(1.0).max(0.0);
        Some((track, confidence, total_score))
    } else {
        None
    }
}

/// Extract qualifiers (text in parentheses) and return (base_name, qualifiers)
fn extract_qualifiers(text: &str) -> (String, Vec<String>) {
    let mut base = text.to_string();
    let mut qualifiers = Vec::new();

    // Extract all text in parentheses
    while let Some(start) = base.find('(') {
        if let Some(end) = base[start..].find(')') {
            let end_idx = start + end;
            let qualifier = base[start + 1..end_idx].trim().to_lowercase();

            // Only keep meaningful qualifiers (not years, catalog numbers, etc.)
            if is_meaningful_qualifier(&qualifier) {
                qualifiers.push(qualifier);
            }

            base = format!("{}{}", &base[..start], &base[end_idx + 1..]);
        } else {
            break;
        }
    }

    // Also check square brackets (but only for removal, not as qualifiers)
    while let Some(start) = base.find('[') {
        if let Some(end) = base[start..].find(']') {
            let end_idx = start + end;
            base = format!("{}{}", &base[..start], &base[end_idx + 1..]);
        } else {
            break;
        }
    }

    // Clean the base name
    base = base.split_whitespace().collect::<Vec<_>>().join(" ");

    (base.trim().to_string(), qualifiers)
}

/// Check if a qualifier is meaningful (version info, not metadata)
fn is_meaningful_qualifier(text: &str) -> bool {
    let meaningful_keywords = [
        "version",
        "remix",
        "mix",
        "edit",
        "live",
        "acoustic",
        "demo",
        "instrumental",
        "karaoke",
        "reprise",
        "cover",
        "radio",
        "extended",
        "short",
        "album",
        "single",
        "feat",
        "featuring",
        "with",
        "explicit",
        "clean",
        "remaster",
        "remastered",
        "anniversary",
        "deluxe",
        "ida",
    ];

    // Check if it contains any meaningful keywords
    let text_lower = text.to_lowercase();
    if meaningful_keywords.iter().any(|kw| text_lower.contains(kw)) {
        return true;
    }

    // Reject if it's just a year
    if text.chars().all(|c| c.is_numeric()) && text.len() == 4 {
        return false;
    }

    // Reject if it looks like a catalog number or ID
    if text.len() < 20 && text.chars().filter(|c| c.is_alphanumeric()).count() == text.len() {
        // Could be ID, but keep if it has meaningful words
        return text.split_whitespace().count() > 1;
    }

    true
}

fn clean_filename(filename: &str) -> String {
    let mut cleaned = filename.to_string();

    // Remove text in square brackets (YouTube IDs, etc.)
    while let Some(bracket_start) = cleaned.find('[') {
        if let Some(bracket_end) = cleaned[bracket_start..].find(']') {
            let end_idx = bracket_start + bracket_end + 1;
            cleaned = format!("{}{}", &cleaned[..bracket_start], &cleaned[end_idx..]);
        } else {
            break;
        }
    }

    // Convert to lowercase
    cleaned = cleaned.to_lowercase();

    // Normalize separators
    cleaned = cleaned
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '(' || c == ')' {
                c
            } else {
                ' '
            }
        })
        .collect();

    // Collapse spaces
    cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");

    cleaned.trim().to_string()
}

fn format_duration(ms: u32) -> String {
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}
