use std::path::Path;

use qoget::models::{Album, AlbumId, Artist, DiscNumber, Track, TrackId, TrackNumber};
use qoget::path::{sanitize_component, track_path};

fn make_album(artist: &str, title: &str, media_count: u8) -> Album {
    Album {
        id: AlbumId("test-album".to_string()),
        title: title.to_string(),
        version: None,
        artist: Artist { id: 1, name: artist.to_string() },
        media_count,
        tracks_count: 10,
        tracks: None,
    }
}

fn make_track(title: &str, number: u8, disc: u8, performer: &str) -> Track {
    Track {
        id: TrackId(1000),
        title: title.to_string(),
        track_number: TrackNumber(number),
        media_number: DiscNumber(disc),
        duration: 200,
        performer: Artist { id: 2, name: performer.to_string() },
        isrc: None,
    }
}

#[test]
fn single_disc_album() {
    let album = make_album("Pink Floyd", "The Dark Side of the Moon", 1);
    let track = make_track("Breathe", 2, 1, "Pink Floyd");
    let base = Path::new("/music");

    let path = track_path(base, &album, &track);
    assert_eq!(
        path,
        Path::new("/music/Pink Floyd/The Dark Side of the Moon/02 - Breathe.mp3")
    );
}

#[test]
fn multi_disc_album() {
    let album = make_album("The Beatles", "White Album", 2);
    let track = make_track("Birthday", 1, 2, "The Beatles");
    let base = Path::new("/music");

    let path = track_path(base, &album, &track);
    assert_eq!(
        path,
        Path::new("/music/The Beatles/White Album/Disc 2/01 - Birthday.mp3")
    );
}

#[test]
fn compilation_album() {
    let album = make_album("Various Artists", "Jazz Classics", 1);
    let track = make_track("So What", 1, 1, "Miles Davis");
    let base = Path::new("/music");

    let path = track_path(base, &album, &track);
    assert_eq!(
        path,
        Path::new("/music/Various Artists/Jazz Classics/01 - Miles Davis - So What.mp3")
    );
}

#[test]
fn sanitize_slashes_and_colons() {
    assert_eq!(sanitize_component("AC/DC"), "AC-DC");
    assert_eq!(sanitize_component("foo\\bar"), "foo-bar");
    assert_eq!(sanitize_component("Title: Subtitle"), "Title- Subtitle");
}

#[test]
fn sanitize_removes_forbidden_chars() {
    assert_eq!(sanitize_component("What?"), "What");
    assert_eq!(sanitize_component("Star*"), "Star");
    assert_eq!(sanitize_component("He said \"hello\""), "He said hello");
    assert_eq!(sanitize_component("<tag>"), "tag");
    assert_eq!(sanitize_component("a|b"), "ab");
}

#[test]
fn sanitize_leading_dot() {
    assert_eq!(sanitize_component(".hidden"), "hidden");
    assert_eq!(sanitize_component("...dots"), "dots");
}

#[test]
fn sanitize_consecutive_spaces() {
    assert_eq!(sanitize_component("a  b   c"), "a b c");
}

#[test]
fn sanitize_truncates_to_255_bytes() {
    let long = "a".repeat(300);
    let result = sanitize_component(&long);
    assert!(result.len() <= 255);
    assert_eq!(result.len(), 255);
}
