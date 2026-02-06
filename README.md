[![CI](https://github.com/nilutz/musictagger_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/nilutz/musictagger_rs/actions/workflows/ci.yml)

[![Release](https://github.com/nilutz/musictagger_rs/actions/workflows/release.yml/badge.svg)](https://github.com/nilutz/musictagger_rs/actions/workflows/release.yml)

# MusicBrainz MP3 Tagger

A CLI tool to tag MP3 files with metadata from MusicBrainz, similar to beets.

## Installation

```bash
cargo build --release
```


## Usage

### MusicBrainz Mode (Album ID)

Tag files using a MusicBrainz release ID:

```bash
# Basic usage
musictagger_rs --path /path/to/music/folder --album-id <MBID>

# Dry run (preview matches without writing)
musictagger_rs --path /path/to/music/folder --album-id <MBID> --dry-run

# Auto-confirm without prompting
musictagger_rs --path /path/to/music/folder --album-id <MBID> --yes
```

### Manual Mode

Interactively tag files without MusicBrainz lookup. Useful for downloaded singles or compilations:

```bash
# Manual tagging mode
musictagger_rs --path /path/to/music/folder --manual

# With dry run
musictagger_rs --path /path/to/music/folder --manual --dry-run
```

In manual mode, the tool will:
1. Prompt for album title (defaults to directory name) and album artist (defaults to "Various Artists")
2. Auto-detect cover art images in the directory (cover.jpg, folder.png, etc.)
3. For each MP3 file, prompt for artist and title (suggests from existing tags or filename)
4. Show a summary and confirm before writing tags