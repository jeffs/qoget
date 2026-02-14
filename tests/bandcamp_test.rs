use std::collections::HashMap;

use qoget::bandcamp::{parse_zip_track_filename, to_purchase_list, BandcampPurchases};
use qoget::models::{
    BandcampCollectionItem, BandcampCollectionResponse, BandcampDownloadFormat,
    BandcampDownloadInfo,
};

// --- BandcampCollectionResponse deserialization ---

#[test]
fn deserialize_collection_response() {
    let json = r#"{
        "more_available": true,
        "last_token": "1707955200:1234567890:a::",
        "redownload_urls": {
            "a1234567": "https://bandcamp.com/download/album?id=1234567&sig=abc",
            "t7654321": "https://bandcamp.com/download/track?id=7654321&sig=def"
        },
        "items": [
            {
                "band_name": "Artist Name",
                "item_title": "Album Title",
                "item_id": 1234567,
                "item_type": "album",
                "sale_item_type": "a",
                "sale_item_id": 1234567,
                "token": "1707955200:1234567890:a::"
            }
        ]
    }"#;

    let resp: BandcampCollectionResponse = serde_json::from_str(json).unwrap();
    assert!(resp.more_available);
    assert_eq!(resp.last_token, "1707955200:1234567890:a::");
    assert_eq!(resp.redownload_urls.len(), 2);
    assert_eq!(
        resp.redownload_urls["a1234567"],
        "https://bandcamp.com/download/album?id=1234567&sig=abc"
    );
    assert_eq!(resp.items.len(), 1);
    assert_eq!(resp.items[0].band_name, "Artist Name");
    assert_eq!(resp.items[0].item_title, "Album Title");
    assert_eq!(resp.items[0].item_id, 1234567);
    assert_eq!(resp.items[0].sale_item_type, "a");
}

#[test]
fn deserialize_empty_collection_response() {
    let json = r#"{
        "more_available": false,
        "last_token": "",
        "redownload_urls": {},
        "items": []
    }"#;

    let resp: BandcampCollectionResponse = serde_json::from_str(json).unwrap();
    assert!(!resp.more_available);
    assert!(resp.items.is_empty());
    assert!(resp.redownload_urls.is_empty());
}

// --- BandcampDownloadInfo deserialization ---

#[test]
fn deserialize_download_info() {
    let json = r#"{
        "item_id": 1234567,
        "title": "Album Title",
        "artist": "Artist Name",
        "download_type": "a",
        "downloads": {
            "aac-hi": { "url": "https://popplers5.bandcamp.com/download/album?enc=aac-hi&id=123", "size_mb": "90.5MB" },
            "mp3-320": { "url": "https://popplers5.bandcamp.com/download/album?enc=mp3-320&id=123", "size_mb": "120.1MB" },
            "flac": { "url": "https://popplers5.bandcamp.com/download/album?enc=flac&id=123", "size_mb": "350.2MB" }
        }
    }"#;

    let info: BandcampDownloadInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.item_id, 1234567);
    assert_eq!(info.title, "Album Title");
    assert_eq!(info.artist, "Artist Name");
    assert_eq!(info.download_type, "a");
    assert_eq!(info.downloads.len(), 3);
    assert!(info.downloads.contains_key("aac-hi"));
    assert!(info.downloads.contains_key("mp3-320"));
    assert!(info.downloads.contains_key("flac"));
    assert_eq!(info.downloads["aac-hi"].size_mb, "90.5MB");
}

// --- aac_hi_url extraction ---

#[test]
fn aac_hi_url_found() {
    let mut downloads = HashMap::new();
    downloads.insert(
        "aac-hi".to_string(),
        BandcampDownloadFormat {
            url: "https://example.com/aac".to_string(),
            size_mb: "90MB".to_string(),
        },
    );
    downloads.insert(
        "mp3-320".to_string(),
        BandcampDownloadFormat {
            url: "https://example.com/mp3".to_string(),
            size_mb: "120MB".to_string(),
        },
    );

    let info = BandcampDownloadInfo {
        item_id: 1,
        title: "Test".to_string(),
        artist: "Artist".to_string(),
        download_type: "a".to_string(),
        downloads,
    };

    let url = qoget::bandcamp::aac_hi_url(&info).unwrap();
    assert_eq!(url, "https://example.com/aac");
}

#[test]
fn aac_hi_url_missing() {
    let mut downloads = HashMap::new();
    downloads.insert(
        "mp3-320".to_string(),
        BandcampDownloadFormat {
            url: "https://example.com/mp3".to_string(),
            size_mb: "120MB".to_string(),
        },
    );

    let info = BandcampDownloadInfo {
        item_id: 1,
        title: "Test Album".to_string(),
        artist: "Test Artist".to_string(),
        download_type: "a".to_string(),
        downloads,
    };

    let err = qoget::bandcamp::aac_hi_url(&info).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("aac-hi"), "error should mention aac-hi: {msg}");
    assert!(
        msg.contains("mp3-320"),
        "error should list available formats: {msg}"
    );
}

// --- ZIP filename parsing ---

#[test]
fn parse_standard_filename() {
    let (num, title) = parse_zip_track_filename("01 Dream House.m4a");
    assert_eq!(num, 1);
    assert_eq!(title, "Dream House");
}

#[test]
fn parse_dash_separator() {
    let (num, title) = parse_zip_track_filename("03 - Sunbather.m4a");
    assert_eq!(num, 3);
    assert_eq!(title, "Sunbather");
}

#[test]
fn parse_dot_separator() {
    let (num, title) = parse_zip_track_filename("12. The Pecan Tree.m4a");
    assert_eq!(num, 12);
    assert_eq!(title, "The Pecan Tree");
}

#[test]
fn parse_no_number() {
    let (num, title) = parse_zip_track_filename("Bonus Track.m4a");
    assert_eq!(num, 0);
    assert_eq!(title, "Bonus Track");
}

#[test]
fn parse_uppercase_extension() {
    let (num, title) = parse_zip_track_filename("05 Windows.M4A");
    assert_eq!(num, 5);
    assert_eq!(title, "Windows");
}

// --- to_purchase_list conversion ---

fn make_item(band: &str, title: &str, item_id: u64, sale_type: &str) -> BandcampCollectionItem {
    BandcampCollectionItem {
        band_name: band.to_string(),
        item_title: title.to_string(),
        item_id,
        item_type: if sale_type == "a" {
            "album".to_string()
        } else {
            "track".to_string()
        },
        sale_item_type: sale_type.to_string(),
        sale_item_id: item_id,
        token: "tok".to_string(),
    }
}

#[test]
fn to_purchase_list_albums() {
    let purchases = BandcampPurchases {
        items: vec![
            make_item("Deafheaven", "Sunbather", 100, "a"),
            make_item("Alcest", "Kodama", 200, "a"),
        ],
        redownload_urls: HashMap::new(),
    };

    let pl = to_purchase_list(&purchases);
    assert_eq!(pl.albums.len(), 2);
    assert_eq!(pl.tracks.len(), 0);

    assert_eq!(pl.albums[0].artist.name, "Deafheaven");
    assert_eq!(pl.albums[0].title, "Sunbather");
    assert_eq!(pl.albums[0].id.0, "bc-100");
    assert_eq!(pl.albums[0].media_count, 1);

    assert_eq!(pl.albums[1].artist.name, "Alcest");
    assert_eq!(pl.albums[1].title, "Kodama");
}

#[test]
fn to_purchase_list_tracks() {
    let purchases = BandcampPurchases {
        items: vec![make_item("Artist", "Single Track", 300, "t")],
        redownload_urls: HashMap::new(),
    };

    let pl = to_purchase_list(&purchases);
    assert_eq!(pl.albums.len(), 0);
    assert_eq!(pl.tracks.len(), 1);

    assert_eq!(pl.tracks[0].title, "Single Track");
    assert_eq!(pl.tracks[0].id.0, 300);
    assert_eq!(pl.tracks[0].track_number.0, 1);
}

#[test]
fn to_purchase_list_mixed() {
    let purchases = BandcampPurchases {
        items: vec![
            make_item("Band A", "Album One", 100, "a"),
            make_item("Band B", "Cool Track", 200, "t"),
            make_item("Band C", "Album Two", 300, "a"),
        ],
        redownload_urls: HashMap::new(),
    };

    let pl = to_purchase_list(&purchases);
    assert_eq!(pl.albums.len(), 2);
    assert_eq!(pl.tracks.len(), 1);
}

#[test]
fn to_purchase_list_unknown_type_skipped() {
    let purchases = BandcampPurchases {
        items: vec![make_item("Band", "Merch Item", 400, "m")],
        redownload_urls: HashMap::new(),
    };

    let pl = to_purchase_list(&purchases);
    assert_eq!(pl.albums.len(), 0);
    assert_eq!(pl.tracks.len(), 0);
}
