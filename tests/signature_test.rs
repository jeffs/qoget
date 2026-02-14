use qoget::client::generate_request_sig;

#[test]
fn signature_with_known_inputs() {
    let sig = generate_request_sig(
        216020864,
        5,
        "1707900000",
        "abcdef1234567890abcdef1234567890",
    );

    // Verify it's a 32-char hex string (MD5 digest)
    assert_eq!(sig.len(), 32);
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));

    // Verify determinism: same inputs produce same output
    let sig2 = generate_request_sig(
        216020864,
        5,
        "1707900000",
        "abcdef1234567890abcdef1234567890",
    );
    assert_eq!(sig, sig2);
}

#[test]
fn signature_always_uses_intentstream() {
    // The signature string always contains "intentstream" regardless of actual intent.
    // We verify this by computing the expected MD5 directly.
    let track_id: u64 = 216020864;
    let format_id: u8 = 5;
    let timestamp = "1707900000";
    let secret = "testsecret";

    let expected_input = format!(
        "trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{secret}"
    );
    let expected_sig = format!("{:x}", md5::compute(expected_input.as_bytes()));

    let actual = generate_request_sig(track_id, format_id, timestamp, secret);
    assert_eq!(actual, expected_sig);
}

#[test]
fn different_inputs_produce_different_signatures() {
    let sig_a = generate_request_sig(100, 5, "1000", "secret_a");
    let sig_b = generate_request_sig(200, 5, "1000", "secret_a");
    let sig_c = generate_request_sig(100, 6, "1000", "secret_a");
    let sig_d = generate_request_sig(100, 5, "2000", "secret_a");
    let sig_e = generate_request_sig(100, 5, "1000", "secret_b");

    assert_ne!(sig_a, sig_b);
    assert_ne!(sig_a, sig_c);
    assert_ne!(sig_a, sig_d);
    assert_ne!(sig_a, sig_e);
}
