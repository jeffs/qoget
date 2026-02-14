use std::path::{Path, PathBuf};

use crate::models::{Album, Track};

/// Replace or remove characters that are invalid or problematic in filesystem paths.
pub fn sanitize_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '/' | '\\' | ':' => out.push('-'),
            '*' | '?' | '"' | '<' | '>' | '|' => {}
            _ => out.push(ch),
        }
    }

    // Trim whitespace
    let trimmed = out.trim();

    // Remove leading dots
    let trimmed = trimmed.trim_start_matches('.');

    // Collapse consecutive spaces
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch == ' ' {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(ch);
            prev_space = false;
        }
    }

    // Truncate to 255 bytes (on a char boundary)
    if result.len() > 255 {
        let mut end = 255;
        while end > 0 && !result.is_char_boundary(end) {
            end -= 1;
        }
        result.truncate(end);
    }

    result
}

/// Build the target path for a track file:
///   base / album_artist / album_title [/ Disc N] / NN - [Track Artist - ] Title{ext}
pub fn track_path(base: &Path, album: &Album, track: &Track, ext: &str) -> PathBuf {
    let artist_dir = sanitize_component(&album.artist.name);
    let album_dir = sanitize_component(&album.title);

    let mut path = base.join(&artist_dir).join(&album_dir);

    // Multi-disc: add "Disc N" subdirectory
    if album.media_count > 1 {
        path = path.join(format!("Disc {}", track.media_number));
    }

    // Build filename
    let track_title = sanitize_component(&track.title);
    let is_compilation = track.performer.name != album.artist.name;

    let num = track.track_number.0;
    let filename = if is_compilation {
        let track_artist = sanitize_component(&track.performer.name);
        format!("{num:02} - {track_artist} - {track_title}{ext}")
    } else {
        format!("{num:02} - {track_title}{ext}")
    };

    path.join(filename)
}
