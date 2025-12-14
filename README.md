# MusicBrainz MP3 Tagger

A CLI tool to tag MP3 files with metadata from MusicBrainz, similar to beets.

## Installation

```bash
cargo build --release
```


# Basic usage
musictagger_rs --path /path/to/music/folder --album-id <MBID>

# Dry run (preview matches without writing)
musictagger_rs --path /path/to/music/folder --album-id <MBID> --dry-run

# Auto-confirm without prompting
musictagger_rs --path /path/to/music/folder --album-id <MBID> --yes