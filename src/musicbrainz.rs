// src/musicbrainz.rs
use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";
const COVERART_API_BASE: &str = "https://coverartarchive.org";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct MusicBrainzClient {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub date: Option<String>,
    pub tracks: Vec<Track>,
    pub total_tracks: u32,
    pub album_artist_id: Option<String>,
    pub media_count: usize,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: String,
    pub position: u32,
    pub title: String,
    pub artist: String,
    pub length: Option<u32>, // in milliseconds
    pub recording_id: String,
    pub disc_number: u32,
    pub disc_title: Option<String>,
}

#[derive(Deserialize, Debug)]
struct MBRelease {
    id: String,
    title: String,
    date: Option<String>,
    #[serde(rename = "artist-credit")]
    artist_credit: Vec<ArtistCredit>,
    media: Vec<Media>,
}

#[derive(Deserialize, Debug)]
struct ArtistCredit {
    artist: Artist,
}

#[derive(Deserialize, Debug)]
struct Artist {
    id: String,
    name: String,
}

#[derive(Deserialize, Debug)]
struct Media {
    position: Option<u32>,
    title: Option<String>,
    tracks: Vec<MBTrack>,
}

#[derive(Deserialize, Debug)]
struct MBTrack {
    id: String,
    position: u32,
    title: String,
    length: Option<u32>,
    recording: Recording,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<ArtistCredit>>,
}

#[derive(Deserialize, Debug)]
struct Recording {
    id: String,
}

#[derive(Deserialize, Debug)]
struct CoverArtResponse {
    images: Vec<CoverArtImage>,
}

#[derive(Deserialize, Debug)]
struct CoverArtImage {
    front: bool,
    image: String,
    thumbnails: Option<CoverArtThumbnails>,
}

#[derive(Deserialize, Debug)]
struct CoverArtThumbnails {
    #[serde(rename = "500")]
    small: Option<String>,
    #[serde(rename = "1200")]
    large: Option<String>,
}

impl MusicBrainzClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(1)
            .tcp_keepalive(Duration::from_secs(60))
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::ACCEPT,
                    reqwest::header::HeaderValue::from_static("application/json"),
                );
                headers
            })
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    pub async fn get_release(&self, release_id: &str) -> Result<Album> {
        let url = format!(
            "{}/release/{}?inc=artist-credits+recordings&fmt=json",
            MB_API_BASE, release_id
        );

        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;

            if attempts > 1 {
                let wait_time = Duration::from_millis(1000 * (2_u64.pow(attempts - 1)));
                tokio::time::sleep(wait_time).await;
            } else {
                tokio::time::sleep(Duration::from_millis(1100)).await;
            }

            let response = match self
                .client
                .get(&url)
                .header("User-Agent", USER_AGENT)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) if attempts < max_attempts => {
                    eprintln!(
                        "Request failed (attempt {}/{}): {}",
                        attempts, max_attempts, e
                    );
                    continue;
                }
                Err(e) => {
                    return Err(e).context("Failed to send request to MusicBrainz");
                }
            };

            let status = response.status();

            if (status == reqwest::StatusCode::SERVICE_UNAVAILABLE
                || status == reqwest::StatusCode::TOO_MANY_REQUESTS)
                && attempts < max_attempts
            {
                eprintln!(
                    "Rate limited, retrying... (attempt {}/{})",
                    attempts, max_attempts
                );
                continue;
            }

            if !status.is_success() {
                let error_body = response.text().await.unwrap_or_default();
                anyhow::bail!("MusicBrainz API error {}: {}", status, error_body);
            }

            let text = response
                .text()
                .await
                .context("Failed to read response body")?;

            let mb_release: MBRelease = serde_json::from_str(&text)
                .with_context(|| format!("Failed to parse MusicBrainz response. Body: {}", text))?;

            return self.parse_release(mb_release);
        }
    }

    pub async fn get_cover_art(&self, release_id: &str) -> Result<Vec<u8>> {
        tokio::time::sleep(Duration::from_millis(1100)).await;

        let url = format!("{}/release/{}", COVERART_API_BASE, release_id);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .context("Failed to request cover art")?;

        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                anyhow::bail!("No cover art found for this release");
            }
            anyhow::bail!("Cover Art Archive returned status: {}", response.status());
        }

        let cover_art_response: CoverArtResponse = response
            .json()
            .await
            .context("Failed to parse cover art response")?;

        let front_image = cover_art_response
            .images
            .iter()
            .find(|img| img.front)
            .or_else(|| cover_art_response.images.first())
            .context("No images found in response")?;

        let image_url = front_image
            .thumbnails
            .as_ref()
            .and_then(|t| t.large.as_ref().or(t.small.as_ref()))
            .unwrap_or(&front_image.image);

        tokio::time::sleep(Duration::from_millis(500)).await;

        let image_response = self
            .client
            .get(image_url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .context("Failed to download cover art image")?;

        if !image_response.status().is_success() {
            anyhow::bail!("Failed to download image: {}", image_response.status());
        }

        let image_bytes = image_response
            .bytes()
            .await
            .context("Failed to read image bytes")?;

        self.resize_if_needed(image_bytes.to_vec())
    }

    fn resize_if_needed(&self, image_data: Vec<u8>) -> Result<Vec<u8>> {
        const MAX_SIZE: u32 = 1200;
        const MAX_BYTES: usize = 1024 * 1024;

        if image_data.len() <= MAX_BYTES {
            if let Ok(img) = image::load_from_memory(&image_data) {
                if img.width() <= MAX_SIZE && img.height() <= MAX_SIZE {
                    return Ok(image_data);
                }
            } else {
                return Ok(image_data);
            }
        }

        let img =
            image::load_from_memory(&image_data).context("Failed to decode image for resizing")?;

        let resized = img.resize(MAX_SIZE, MAX_SIZE, image::imageops::FilterType::Lanczos3);

        let mut output = std::io::Cursor::new(Vec::new());
        resized
            .write_to(&mut output, image::ImageOutputFormat::Jpeg(90))
            .context("Failed to encode resized image")?;

        Ok(output.into_inner())
    }

    fn parse_release(&self, mb_release: MBRelease) -> Result<Album> {
        let album_artist = mb_release
            .artist_credit
            .first()
            .map(|ac| ac.artist.name.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album_artist_id = mb_release
            .artist_credit
            .first()
            .map(|ac| ac.artist.id.clone());

        let mut all_tracks = Vec::new();
        let media_count = mb_release.media.len();

        for (medium_idx, medium) in mb_release.media.into_iter().enumerate() {
            let disc_number = medium.position.unwrap_or((medium_idx + 1) as u32);
            let disc_title = medium.title.clone();

            for mb_track in medium.tracks {
                let track_artist = mb_track
                    .artist_credit
                    .as_ref()
                    .and_then(|ac| ac.first())
                    .map(|ac| ac.artist.name.clone())
                    .unwrap_or_else(|| album_artist.clone());

                all_tracks.push(Track {
                    id: mb_track.id,
                    position: mb_track.position,
                    title: mb_track.title,
                    artist: track_artist,
                    length: mb_track.length,
                    recording_id: mb_track.recording.id,
                    disc_number,
                    disc_title: disc_title.clone(),
                });
            }
        }

        let total_tracks = all_tracks.len() as u32;

        Ok(Album {
            id: mb_release.id,
            title: mb_release.title,
            artist: album_artist,
            date: mb_release.date,
            tracks: all_tracks,
            total_tracks,
            album_artist_id,
            media_count,
        })
    }
}
