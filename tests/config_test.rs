use qoget::config::{QobuzState, parse_toml_config};

#[test]
fn new_format_qobuz_only() {
    let cfg = parse_toml_config(
        r#"
[qobuz]
username = "user@example.com"
password = "secret"
"#,
    )
    .unwrap();
    let q = cfg.qobuz.ready().expect("qobuz should be configured");
    assert_eq!(q.username, "user@example.com");
    assert_eq!(q.password, "secret");
    assert!(q.app_id.is_none());
    assert!(cfg.bandcamp.is_none());
}

#[test]
fn new_format_both_services() {
    let cfg = parse_toml_config(
        r#"
[qobuz]
username = "user@example.com"
password = "secret"

[bandcamp]
identity_cookie = "6%09abc"
"#,
    )
    .unwrap();
    assert!(cfg.qobuz.ready().is_some());
    let b = cfg.bandcamp.expect("bandcamp should be configured");
    assert_eq!(b.identity_cookie, "6%09abc");
}

#[test]
fn old_format_bare_keys() {
    let cfg = parse_toml_config(
        r#"
username = "user@example.com"
password = "secret"
"#,
    )
    .unwrap();
    let q = cfg.qobuz.ready().expect("qobuz via bare keys");
    assert_eq!(q.username, "user@example.com");
    assert_eq!(q.password, "secret");
    assert!(cfg.bandcamp.is_none());
}

#[test]
fn mixed_bare_keys_and_bandcamp_section() {
    let cfg = parse_toml_config(
        r#"
username = "user@example.com"
password = "secret"

[bandcamp]
identity_cookie = "cookie-val"
"#,
    )
    .unwrap();
    assert!(cfg.qobuz.ready().is_some());
    assert!(cfg.bandcamp.is_some());
}

#[test]
fn bandcamp_only() {
    let cfg = parse_toml_config(
        r#"
[bandcamp]
identity_cookie = "cookie-val"
"#,
    )
    .unwrap();
    assert!(matches!(cfg.qobuz, QobuzState::NotConfigured));
    let b = cfg.bandcamp.expect("bandcamp should be configured");
    assert_eq!(b.identity_cookie, "cookie-val");
}

#[test]
fn empty_config() {
    let cfg = parse_toml_config("").unwrap();
    assert!(matches!(cfg.qobuz, QobuzState::NotConfigured));
    assert!(cfg.bandcamp.is_none());
}

#[test]
fn section_takes_precedence_over_bare_keys() {
    let cfg = parse_toml_config(
        r#"
username = "bare@example.com"
password = "bare-pass"

[qobuz]
username = "section@example.com"
password = "section-pass"
"#,
    )
    .unwrap();
    let q = cfg.qobuz.ready().expect("qobuz");
    assert_eq!(q.username, "section@example.com");
    assert_eq!(q.password, "section-pass");
}

#[test]
fn app_id_and_secret_from_section() {
    let cfg = parse_toml_config(
        r#"
[qobuz]
username = "user@example.com"
password = "secret"
app_id = "123456789"
app_secret = "abc-secret"
"#,
    )
    .unwrap();
    let q = cfg.qobuz.ready().expect("qobuz");
    assert_eq!(q.app_id.as_deref(), Some("123456789"));
    assert_eq!(q.app_secret.as_deref(), Some("abc-secret"));
}

#[test]
fn app_id_from_bare_keys() {
    let cfg = parse_toml_config(
        r#"
username = "user@example.com"
password = "secret"
app_id = "987654321"
app_secret = "xyz-secret"
"#,
    )
    .unwrap();
    let q = cfg.qobuz.ready().expect("qobuz");
    assert_eq!(q.app_id.as_deref(), Some("987654321"));
    assert_eq!(q.app_secret.as_deref(), Some("xyz-secret"));
}

#[test]
fn empty_strings_treated_as_missing() {
    let cfg = parse_toml_config(
        r#"
[qobuz]
username = ""
password = "secret"
"#,
    )
    .unwrap();
    // Empty username → no Qobuz intent
    assert!(matches!(cfg.qobuz, QobuzState::NotConfigured));
}

#[test]
fn username_without_password_is_incomplete() {
    let cfg = parse_toml_config(
        r#"
[qobuz]
username = "user@example.com"
"#,
    )
    .unwrap();
    assert!(matches!(cfg.qobuz, QobuzState::Incomplete));
}

#[test]
fn bare_username_without_password_is_incomplete() {
    let cfg = parse_toml_config(
        r#"
username = "user@example.com"
"#,
    )
    .unwrap();
    assert!(matches!(cfg.qobuz, QobuzState::Incomplete));
}

#[test]
fn empty_bandcamp_cookie_treated_as_missing() {
    let cfg = parse_toml_config(
        r#"
[bandcamp]
identity_cookie = ""
"#,
    )
    .unwrap();
    assert!(cfg.bandcamp.is_none());
}
