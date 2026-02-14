use qoget::models::{Album, AlbumId, FileUrlResponse, LoginResponse, PurchaseResponse, TrackId};

#[test]
fn parse_login_response() {
    let json = r#"{
        "user_auth_token": "abc123token",
        "user": {
            "id": 42,
            "login": "testuser",
            "email": "test@example.com"
        }
    }"#;

    let resp: LoginResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.user_auth_token, "abc123token");
    assert_eq!(resp.user.id, 42);
}

#[test]
fn parse_purchase_response() {
    let json = r#"{
        "albums": {
            "offset": 0,
            "limit": 500,
            "total": 1,
            "items": [
                {
                    "id": "album-123",
                    "title": "Test Album",
                    "version": null,
                    "artist": { "id": 99, "name": "Test Artist" },
                    "media_count": 2,
                    "tracks_count": 14
                }
            ]
        },
        "tracks": {
            "offset": 0,
            "limit": 500,
            "total": 0,
            "items": []
        }
    }"#;

    let resp: PurchaseResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.albums.total, 1);
    assert_eq!(resp.albums.items.len(), 1);
    assert_eq!(resp.albums.items[0].title, "Test Album");
    assert_eq!(resp.albums.items[0].media_count, 2);
    assert_eq!(resp.albums.items[0].tracks_count, 14);
    assert_eq!(resp.tracks.total, 0);
}

#[test]
fn parse_album_with_tracks() {
    let json = r#"{
        "id": "album-456",
        "title": "Full Album",
        "version": "Deluxe Edition",
        "artist": { "id": 10, "name": "Band Name" },
        "media_count": 1,
        "tracks_count": 3,
        "tracks": {
            "offset": 0,
            "limit": 50,
            "total": 3,
            "items": [
                {
                    "id": 216020864,
                    "title": "Track One",
                    "track_number": 1,
                    "media_number": 1,
                    "duration": 240,
                    "performer": { "id": 10, "name": "Band Name" },
                    "isrc": "USMRG2384109"
                },
                {
                    "id": 216020865,
                    "title": "Track Two",
                    "track_number": 2,
                    "media_number": 1,
                    "duration": 180,
                    "performer": { "id": 20, "name": "Guest Artist" },
                    "isrc": null
                },
                {
                    "id": 216020866,
                    "title": "Track Three",
                    "track_number": 3,
                    "media_number": 1,
                    "duration": 300,
                    "performer": { "id": 10, "name": "Band Name" },
                    "isrc": "USMRG2384111"
                }
            ]
        }
    }"#;

    let album: Album = serde_json::from_str(json).unwrap();
    assert_eq!(album.title, "Full Album");
    assert_eq!(album.version, Some("Deluxe Edition".to_string()));
    assert_eq!(album.artist.name, "Band Name");

    let tracks = album.tracks.unwrap();
    assert_eq!(tracks.items.len(), 3);
    assert_eq!(tracks.items[0].track_number.0, 1);
    assert_eq!(tracks.items[0].media_number.0, 1);
    assert_eq!(tracks.items[1].performer.name, "Guest Artist");
    assert_eq!(tracks.items[2].isrc, Some("USMRG2384111".to_string()));
}

#[test]
fn parse_file_url_response() {
    let json = r#"{
        "track_id": 216020864,
        "url": "https://streaming-qobuz-std.akamaized.net/file?uid=test",
        "format_id": 5,
        "mime_type": "audio/mpeg"
    }"#;

    let resp: FileUrlResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.track_id, 216020864);
    assert!(resp.url.starts_with("https://"));
    assert_eq!(resp.format_id, 5);
    assert_eq!(resp.mime_type, "audio/mpeg");
}

#[test]
fn track_id_newtype_deserializes() {
    let json = "216020864";
    let id: TrackId = serde_json::from_str(json).unwrap();
    assert_eq!(id.0, 216020864);
    assert_eq!(format!("{}", id), "216020864");
}

#[test]
fn album_id_newtype_deserializes() {
    let json = "\"album-789\"";
    let id: AlbumId = serde_json::from_str(json).unwrap();
    assert_eq!(id.0, "album-789");
    assert_eq!(format!("{}", id), "album-789");
}
