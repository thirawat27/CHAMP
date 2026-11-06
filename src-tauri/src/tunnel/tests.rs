use super::*;

#[test]
fn extracts_ngrok_url_from_started_tunnel_line() {
    let line = r#"t=2026-10-28T21:00:00+0700 lvl=info msg="started tunnel" obj=tunnels name=command_line addr=http://127.0.0.1:8080 url=https://demo-random.ngrok-free.app"#;
    assert_eq!(
        extract_ngrok_url(line).as_deref(),
        Some("https://demo-random.ngrok-free.app")
    );
}

#[test]
fn ignores_lines_without_public_url() {
    assert!(extract_ngrok_url("lvl=info msg=\"client session established\"").is_none());
    assert!(extract_ngrok_url("lvl=info addr=http://127.0.0.1:8080").is_none());
}

#[test]
fn request_lines_cannot_set_the_public_url() {
    let line = r#"lvl=info msg="join connections" obj=join url=https://attacker.example"#;
    assert!(extract_ngrok_url(line).is_none());
}

#[test]
fn stop_cancels_a_start_in_progress() {
    let before = STOP_GENERATION.load(Ordering::SeqCst);
    stop_https_tunnel().expect("stop failed");
    assert!(STOP_GENERATION.load(Ordering::SeqCst) > before);
}

#[test]
fn explains_missing_authtoken() {
    let line = r#"t=2026-10-28T21:00:00+0700 lvl=eror msg="failed to reconnect session" obj=tunnels.session err="authentication failed: Usage of ngrok requires a verified account and authtoken.\n\nERR_NGROK_4018\n""#;
    let error = extract_ngrok_error(line).unwrap();
    assert!(error.contains("authtoken"));
    assert!(error.contains(NGROK_SIGNUP_URL));
}

#[test]
fn ignores_info_lines_as_errors() {
    assert!(extract_ngrok_error("lvl=info msg=\"no configuration paths supplied\"").is_none());
}

#[test]
fn reads_authtoken_from_ngrok_config() {
    assert!(config_has_authtoken(
        "version: \"3\"\nagent:\n    authtoken: abc123\n"
    ));
    assert!(!config_has_authtoken(
        "version: \"3\"\nagent:\n    authtoken: \"\"\n"
    ));
}
