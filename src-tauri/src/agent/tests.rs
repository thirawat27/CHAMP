use super::handlers::*;
use super::http::*;
use super::tools::*;
use super::*;
use hyper::header::{HeaderMap, HeaderValue, AUTHORIZATION, ORIGIN};
use hyper::StatusCode;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn headers(pairs: &[(hyper::header::HeaderName, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(name.clone(), HeaderValue::from_str(value).unwrap());
    }
    map
}

#[test]
fn requests_need_the_exact_bearer_token() {
    let bearer = format!("Bearer {TOKEN}");
    assert!(check_headers(&headers(&[(AUTHORIZATION, &bearer)]), TOKEN).is_ok());
    assert_eq!(
        check_headers(&headers(&[]), TOKEN).unwrap_err().0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        check_headers(&headers(&[(AUTHORIZATION, "Bearer wrong")]), TOKEN)
            .unwrap_err()
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        check_headers(&headers(&[(AUTHORIZATION, TOKEN)]), TOKEN)
            .unwrap_err()
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn browser_requests_are_refused_even_with_token() {
    let bearer = format!("Bearer {TOKEN}");
    let result = check_headers(
        &headers(&[(AUTHORIZATION, &bearer), (ORIGIN, "https://evil.example")]),
        TOKEN,
    );
    assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
}

#[test]
fn read_only_mode_hides_write_tools() {
    let names = |read_only| {
        tool_definitions(read_only)
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let full = names(false);
    let limited = names(true);
    assert!(WRITE_TOOLS
        .iter()
        .all(|name| full.iter().any(|t| t == name)));
    assert!(WRITE_TOOLS
        .iter()
        .all(|name| !limited.iter().any(|t| t == name)));
    assert!(limited.iter().any(|t| t == "diagnose_service"));
}

#[test]
fn protocol_version_is_negotiated() {
    assert_eq!(
        negotiate_protocol(&json!({ "protocolVersion": "2025-03-26" })),
        "2025-03-26"
    );
    assert_eq!(
        negotiate_protocol(&json!({ "protocolVersion": "1999-01-01" })),
        "2025-06-18"
    );
    assert_eq!(initialize_result(&json!({}))["serverInfo"]["name"], "champ");
}

#[test]
fn generated_tokens_are_random_hex() {
    let a = generate_token().unwrap();
    let b = generate_token().unwrap();
    assert_eq!(a.len(), 64);
    assert!(a.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_ne!(a, b);
}

#[test]
fn every_tool_hidden_in_read_only_mode_is_a_write_tool() {
    let limited: Vec<Value> = tool_definitions(true);
    for tool in tool_definitions(false) {
        let name = tool["name"].as_str().unwrap();
        let visible = limited.iter().any(|t| t["name"] == name);
        assert_eq!(!visible, WRITE_TOOLS.contains(&name), "{name}");
        assert_eq!(tool["annotations"]["readOnlyHint"], visible, "{name}");
    }
    for name in ["list_databases", "get_https_preview", "get_environment"] {
        assert!(limited.iter().any(|t| t["name"] == name), "{name}");
    }
}

#[test]
fn self_test_explains_why_it_cannot_connect() {
    let mut status = AgentStatus {
        enabled: false,
        running: false,
        read_only: false,
        port: 47823,
        url: "http://127.0.0.1:47823/mcp".to_string(),
        token: None,
        error: None,
    };
    let off = tauri::async_runtime::block_on(probe(&status)).unwrap_err();
    assert!(off.contains("turned off"), "{off}");
    status.enabled = true;
    status.error = Some("Port 47823 is not available".to_string());
    let failed = tauri::async_runtime::block_on(probe(&status)).unwrap_err();
    assert!(failed.contains("47823"), "{failed}");
}

#[test]
fn tool_schemas_are_objects() {
    for tool in tool_definitions(false) {
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
    }
}
