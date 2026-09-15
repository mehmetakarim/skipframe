//! A stand-in StepperSkip for tests, speaking just enough HTTP on the loopback interface.
//!
//! It enforces the properties the client has to get right, rather than answering whatever it is
//! sent: the PKCE verifier must hash to the challenge the browser carried, refresh tokens rotate,
//! and presenting a rotated-out refresh token is recorded as reuse — which is what the real
//! server punishes by revoking the family.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::pkce::challenge_for;

#[derive(Default)]
pub struct MockState {
    pub code_exchanges: AtomicUsize,
    pub refreshes: AtomicUsize,
    pub reuse_detected: AtomicUsize,
    pub revocations: AtomicUsize,
    expected_challenge: Mutex<Option<String>>,
    current_refresh: Mutex<String>,
    retired_refresh: Mutex<HashSet<String>>,
    valid_access: Mutex<HashSet<String>>,
}

#[derive(Clone)]
pub struct MockServer {
    addr: String,
    pub state: Arc<MockState>,
}

impl MockServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let state = Arc::new(MockState::default());
        let served = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(handle(stream, served.clone()));
            }
        });
        MockServer { addr, state }
    }

    pub fn base_url(&self) -> String {
        // A path prefix, like the XAMPP install at /stepperskipcom.
        format!("http://{}/stepperskipcom", self.addr)
    }

    pub fn set_refresh_token(&self, token: &str) {
        *self.state.current_refresh.lock().unwrap() = token.to_string();
    }
}

/// Play the browser: note the challenge the server would have stored, then come back to the
/// loopback with a code. `forged_state` sends a different state instead of the real one.
pub fn browser_returns(
    mock: MockServer,
    forged_state: Option<&'static str>,
) -> impl FnOnce(&str) -> Result<(), String> {
    move |authorize: &str| {
        let url = url::Url::parse(authorize).map_err(|e| e.to_string())?;
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        *mock.state.expected_challenge.lock().unwrap() = Some(query["code_challenge"].clone());
        let state = forged_state
            .map(str::to_string)
            .unwrap_or(query["state"].clone());
        let target = format!("/callback?code=good-code&state={state}");
        tokio::spawn(visit(query["redirect_uri"].clone(), target));
        Ok(())
    }
}

/// Play a browser whose user pressed "deny".
pub fn browser_denies() -> impl FnOnce(&str) -> Result<(), String> {
    |authorize: &str| {
        let url = url::Url::parse(authorize).map_err(|e| e.to_string())?;
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        let target = format!("/callback?error=access_denied&state={}", query["state"]);
        tokio::spawn(visit(query["redirect_uri"].clone(), target));
        Ok(())
    }
}

async fn visit(redirect_uri: String, target: String) {
    let addr = redirect_uri
        .trim_start_matches("http://")
        .trim_end_matches("/callback")
        .to_string();
    let mut s = TcpStream::connect(&addr).await.unwrap();
    let req = format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\n\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut sink = Vec::new();
    let _ = s.read_to_end(&mut sink).await;
}

async fn handle(mut stream: TcpStream, state: Arc<MockState>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, content_length) = loop {
        let n = match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..pos]).to_string();
            let length = head
                .lines()
                .find_map(|l| {
                    let lower = l.to_ascii_lowercase();
                    lower
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            break (pos + 4, length);
        }
    };
    while buf.len() < head_end + content_length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }

    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let body = buf[head_end..].to_vec();
    let mut request_line = head.lines().next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_string();
    let path = request_line.next().unwrap_or_default().to_string();
    let bearer = head.lines().find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("authorization")
            .then(|| value.trim().strip_prefix("Bearer ").map(str::to_string))?
    });

    let (status, json) = route(&state, &method, &path, &body, bearer.as_deref()).await;
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
}

fn error(code: &str) -> String {
    format!(r#"{{"success":false,"error":{{"code":"{code}","message":"mock","detail":{{}}}}}}"#)
}

fn tokens(access: &str, refresh: &str) -> String {
    format!(
        r#"{{"access_token":"{access}","token_type":"Bearer","expires_in":604800,"refresh_token":"{refresh}","scope":"profile:read company:read video:write"}}"#
    )
}

async fn route(
    state: &MockState,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: Option<&str>,
) -> (u16, String) {
    let path = path.strip_prefix("/stepperskipcom").unwrap_or(path);
    let form: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(body).into_owned().collect();
    let field = |k: &str| form.get(k).map(String::as_str).unwrap_or_default();

    match (method, path) {
        ("POST", "/oauth/token") if field("grant_type") == "authorization_code" => {
            let expected = state.expected_challenge.lock().unwrap().clone();
            let pkce_ok =
                expected.as_deref() == Some(challenge_for(field("code_verifier")).as_str());
            if field("code") != "good-code" || field("client_id") != "skipframe-desktop" || !pkce_ok
            {
                return (400, error("invalid_grant"));
            }
            state.code_exchanges.fetch_add(1, Ordering::SeqCst);
            *state.current_refresh.lock().unwrap() = "rt-0".into();
            state.valid_access.lock().unwrap().insert("at-0".into());
            (200, tokens("at-0", "rt-0"))
        }
        ("POST", "/oauth/token") if field("grant_type") == "refresh_token" => {
            let presented = field("refresh_token").to_string();
            let rotated = {
                let mut current = state.current_refresh.lock().unwrap();
                if *current == presented {
                    let n = state.refreshes.fetch_add(1, Ordering::SeqCst) + 1;
                    state
                        .retired_refresh
                        .lock()
                        .unwrap()
                        .insert(presented.clone());
                    *current = format!("rt-{n}");
                    let access = format!("at-{n}");
                    state.valid_access.lock().unwrap().insert(access.clone());
                    Some((access, current.clone()))
                } else {
                    if state.retired_refresh.lock().unwrap().contains(&presented) {
                        state.reuse_detected.fetch_add(1, Ordering::SeqCst);
                    }
                    None
                }
            };
            // Widen the window in which a second, concurrent refresh would slip through.
            tokio::time::sleep(Duration::from_millis(50)).await;
            match rotated {
                Some((access, refresh)) => (200, tokens(&access, &refresh)),
                None => (400, error("invalid_grant")),
            }
        }
        ("POST", "/oauth/revoke") => {
            state.revocations.fetch_add(1, Ordering::SeqCst);
            (200, r#"{"success":true}"#.into())
        }
        ("GET", resource) if resource.starts_with("/api/v1/") => {
            let authorised = bearer.is_some_and(|t| state.valid_access.lock().unwrap().contains(t));
            if !authorised {
                return (401, error("invalid_token"));
            }
            match resource {
                "/api/v1/me" => (
                    200,
                    r#"{"success":true,"data":{"user":{"id":97,"username":"mert.kaya","display_name":"Mert Kaya","avatar_url":null,"profile_url":"http://localhost/stepperskipcom/@mert.kaya"}}}"#.into(),
                ),
                "/api/v1/companies" => (
                    200,
                    r#"{"success":true,"data":{"companies":[{"id":1,"name":"Teknovada","slug":"teknovada","logo_url":null,"profile_url":"http://localhost/stepperskipcom/company/teknovada","is_active":true,"can_publish":true}]}}"#.into(),
                ),
                "/api/v1/limits" => (
                    200,
                    r#"{"success":true,"data":{"limits":{"max_file_size_bytes":52428800,"max_video_duration_seconds":60,"preferred_chunk_size_bytes":5242880,"allowed_mime_types":["video/mp4"]},"capabilities":{"resumable_upload":true,"company_video_publish":true}}}"#.into(),
                ),
                _ => (404, error("not_found")),
            }
        }
        _ => (404, error("not_found")),
    }
}
