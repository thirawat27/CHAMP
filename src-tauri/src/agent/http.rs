//! The localhost HTTP endpoint and its JSON-RPC envelope.

use super::handlers::respond;
use super::token::constant_time_eq;
use super::Context;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{
    HeaderMap, HeaderValue, ALLOW, AUTHORIZATION, CONTENT_TYPE, ORIGIN, WWW_AUTHENTICATE,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub(super) const MAX_BODY_BYTES: usize = 1024 * 1024;

pub(super) type HttpResponse = Response<Full<Bytes>>;

/// Tools that change the stack; hidden and refused in read-only mode.
// JSON-RPC 2.0 error codes.
pub(super) const RPC_PARSE_ERROR: i64 = -32700;

pub(super) const RPC_INVALID_REQUEST: i64 = -32600;

pub(super) const RPC_METHOD_NOT_FOUND: i64 = -32601;

pub(super) const RPC_INVALID_PARAMS: i64 = -32602;

pub(super) const BIND_RETRY_ATTEMPTS: usize = 10;

pub(super) const BIND_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Reject browser-originated and unauthenticated requests.
pub(super) fn check_headers(
    headers: &HeaderMap,
    token: &str,
) -> Result<(), (StatusCode, &'static str)> {
    if headers.contains_key(ORIGIN) {
        return Err((StatusCode::FORBIDDEN, "Browser requests are not allowed"));
    }
    let presented = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if constant_time_eq(presented.trim().as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            "Missing or invalid bearer token. Copy the setup again from CHAMP Settings -> AI agent connector (the token changes when you create a new one).",
        ))
    }
}

pub(super) fn text_response(status: StatusCode, message: &str) -> HttpResponse {
    let mut response = Response::new(Full::new(Bytes::from(message.to_string())));
    *response.status_mut() = status;
    response
}

pub(super) fn json_response(body: &Value) -> HttpResponse {
    let mut response = Response::new(Full::new(Bytes::from(body.to_string())));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

pub(super) fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub(super) fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub(super) async fn handle(request: Request<Incoming>, ctx: Arc<Context>) -> HttpResponse {
    if request.uri().path() != "/mcp" {
        return text_response(
            StatusCode::NOT_FOUND,
            "Not found. The CHAMP MCP endpoint is /mcp",
        );
    }
    if let Err((status, message)) = check_headers(request.headers(), &ctx.token) {
        let mut response = text_response(status, message);
        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer realm=\"CHAMP\""),
            );
        }
        return response;
    }
    if request.method() != Method::POST {
        let mut response = text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "Use POST (MCP Streamable HTTP, JSON responses only)",
        );
        response
            .headers_mut()
            .insert(ALLOW, HeaderValue::from_static("POST"));
        return response;
    }

    let body = match Limited::new(request.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return text_response(StatusCode::PAYLOAD_TOO_LARGE, "Request too large"),
    };
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => return json_response(&rpc_error(Value::Null, RPC_PARSE_ERROR, "Parse error")),
    };

    match respond(&ctx, &message).await {
        Some(reply) => json_response(&reply),
        None => text_response(StatusCode::ACCEPTED, ""),
    }
}

pub(super) async fn accept_loop(
    listener: TcpListener,
    ctx: Arc<Context>,
    cancel: CancellationToken,
) {
    loop {
        let stream = tokio::select! {
            _ = cancel.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => stream,
                Err(_) => continue,
            },
        };

        let ctx = Arc::clone(&ctx);
        let cancel = cancel.clone();
        tauri::async_runtime::spawn(async move {
            let service = service_fn(move |request| {
                let ctx = Arc::clone(&ctx);
                async move { Ok::<_, Infallible>(handle(request, ctx).await) }
            });
            let connection = http1::Builder::new().serve_connection(TokioIo::new(stream), service);
            tokio::select! {
                _ = connection => {}
                _ = cancel.cancelled() => {}
            }
        });
    }
}

pub(super) async fn bind(port: u16) -> Result<TcpListener, String> {
    // A server that was just cancelled may still hold the port for a moment.
    let mut last_error = None;
    for _ in 0..BIND_RETRY_ATTEMPTS {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
            Ok(listener) => return Ok(listener),
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(BIND_RETRY_DELAY).await;
    }
    Err(format!(
        "Port {} is not available ({}). Another program may be using it; choose a different port.",
        port,
        last_error.map(|e| e.to_string()).unwrap_or_default()
    ))
}
