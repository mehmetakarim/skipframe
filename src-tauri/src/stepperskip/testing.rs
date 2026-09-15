//! A stand-in StepperSkip for tests, speaking just enough HTTP on the loopback interface.
//!
//! It enforces the properties the client has to get right, rather than answering whatever it is
//! sent: the PKCE verifier must hash to the challenge the browser carried, refresh tokens rotate,
//! and presenting a rotated-out refresh token is recorded as reuse — which is what the real
//! server punishes by revoking the family.
//!
//! The upload endpoints follow `Skipframe_api.php` and `Skipframe_upload_service.php`: strictly
//! sequential chunks, 308 until the last, the offset-conflict detail, idempotent publishing — and,
//! when asked, the bug in `append_chunk` where a cut-short chunk is written to disk without being
//! recorded, which then fails the session on the next chunk.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

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
    /// The body of the last page the pretend browser was shown at the loopback.
    last_page: Mutex<Option<String>>,

    // -- uploads --------------------------------------------------------------------------------
    pub creates: AtomicUsize,
    pub puts: AtomicUsize,
    pub status_checks: AtomicUsize,
    pub publishes: AtomicUsize,
    pub posts_created: AtomicUsize,
    uploads: Mutex<HashMap<String, MockUpload>>,
    posts: Mutex<HashMap<String, u64>>,
    put_faults: Mutex<HashMap<usize, Fault>>,
    publish_faults: Mutex<HashMap<usize, Fault>>,
    chunk_bytes: AtomicU64,
    partial_write_bug: AtomicBool,
    reject_on_completion: Mutex<Option<(u16, String)>>,
    corrupt_on_completion: AtomicBool,
    completed_sha256: Mutex<Option<String>>,
}

struct MockUpload {
    company_id: u64,
    total: u64,
    /// `payload.part` on disk.
    file: Vec<u8>,
    /// `received_size` in the database. The two can disagree — that is the bug.
    received: u64,
    status: String,
}

/// Something going wrong with one particular request, counted from 1 per endpoint.
#[derive(Debug, Clone, Copy)]
pub enum Fault {
    /// Close the connection before reading the body: the server never saw the chunk.
    DropBeforeBody,
    /// Read only the first `keep` bytes of the body, then the connection is gone.
    CutBody { keep: usize },
    /// Handle the request completely, then close without answering.
    LoseResponse,
    /// Answer 429 without doing anything.
    RateLimit,
    /// Revoke every access token just before handling the request.
    RevokeTokens,
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

    pub fn set_chunk_bytes(&self, bytes: u64) {
        self.state.chunk_bytes.store(bytes, Ordering::SeqCst);
    }

    /// Whether a cut-short chunk behaves as `append_chunk` does today (partial bytes kept on disk,
    /// not recorded) or as a fixed server would (rolled back).
    pub fn set_partial_write_bug(&self, on: bool) {
        self.state.partial_write_bug.store(on, Ordering::SeqCst);
    }

    pub fn put_fault(&self, nth: usize, fault: Fault) {
        self.state.put_faults.lock().unwrap().insert(nth, fault);
    }

    pub fn publish_fault(&self, nth: usize, fault: Fault) {
        self.state.publish_faults.lock().unwrap().insert(nth, fault);
    }

    /// Fail validation when the last chunk arrives, the way ffprobe rejects a 61-second video.
    pub fn reject_on_completion(&self, status: u16, code: &str) {
        *self.state.reject_on_completion.lock().unwrap() = Some((status, code.to_string()));
    }

    /// Flip one byte of the assembled file before hashing it, as a damaged transfer or storage
    /// fault would.
    pub fn corrupt_on_completion(&self) {
        self.state
            .corrupt_on_completion
            .store(true, Ordering::SeqCst);
    }

    /// SHA-256 of the bytes the server assembled for the last completed upload.
    pub fn completed_sha256(&self) -> Option<String> {
        self.state.completed_sha256.lock().unwrap().clone()
    }

    /// The page the browser landed on. Waits briefly, because the browser task reads it after
    /// the sign-in call has already returned.
    pub async fn last_page(&self) -> String {
        for _ in 0..100 {
            if let Some(page) = self.state.last_page.lock().unwrap().clone() {
                return page;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the browser was never shown a page");
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
        let shown = mock.state.clone();
        let redirect_uri = query["redirect_uri"].clone();
        tokio::spawn(async move {
            let page = visit(redirect_uri, target).await;
            *shown.last_page.lock().unwrap() = Some(page);
        });
        Ok(())
    }
}

/// Play a browser whose user pressed "deny".
pub fn browser_denies() -> impl FnOnce(&str) -> Result<(), String> {
    |authorize: &str| {
        let url = url::Url::parse(authorize).map_err(|e| e.to_string())?;
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        let target = format!("/callback?error=access_denied&state={}", query["state"]);
        tokio::spawn(async move {
            visit(query["redirect_uri"].clone(), target).await;
        });
        Ok(())
    }
}

async fn visit(redirect_uri: String, target: String) -> String {
    let addr = redirect_uri
        .trim_start_matches("http://")
        .trim_end_matches("/callback")
        .to_string();
    let mut s = TcpStream::connect(&addr).await.unwrap();
    let req = format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\n\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut page = Vec::new();
    let _ = s.read_to_end(&mut page).await;
    String::from_utf8_lossy(&page).into_owned()
}

async fn handle(mut stream: TcpStream, state: Arc<MockState>) {
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let head_end = loop {
        let n = match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    let content_length = header("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let content_range = header("content-range");
    let bearer =
        header("authorization").and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
    let mut request_line = head.lines().next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_string();
    let path = request_line.next().unwrap_or_default().to_string();
    let local = path
        .strip_prefix("/stepperskipcom")
        .unwrap_or(&path)
        .to_string();

    let is_put = method == "PUT" && local.starts_with("/api/v1/uploads/");
    let is_publish = method == "POST" && local == "/api/v1/company-posts";
    let fault = if is_put {
        let n = state.puts.fetch_add(1, Ordering::SeqCst) + 1;
        state.put_faults.lock().unwrap().remove(&n)
    } else if is_publish {
        let n = state.publishes.fetch_add(1, Ordering::SeqCst) + 1;
        state.publish_faults.lock().unwrap().remove(&n)
    } else {
        None
    };

    match fault {
        Some(Fault::DropBeforeBody) => return,
        Some(Fault::RateLimit) => {
            respond(&mut stream, 429, &error("rate_limited")).await;
            return;
        }
        Some(Fault::RevokeTokens) => state.valid_access.lock().unwrap().clear(),
        _ => {}
    }

    let wanted = match fault {
        Some(Fault::CutBody { keep }) => keep.min(content_length),
        _ => content_length,
    };
    while buf.len() < head_end + wanted {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let mut body = buf[head_end..].to_vec();
    body.truncate(wanted);

    // The body stopped short — cut by a fault, or the client went away mid-request. PHP carries on
    // with what it read; model that, and answer nothing.
    if body.len() < content_length {
        if is_put {
            partial_put(
                &state,
                &local,
                bearer.as_deref(),
                content_range.as_deref(),
                &body,
            );
        }
        return;
    }

    let (status, json) = route(
        &state,
        &method,
        &local,
        &body,
        bearer.as_deref(),
        content_range.as_deref(),
    )
    .await;
    if matches!(fault, Some(Fault::LoseResponse)) {
        return;
    }
    respond(&mut stream, status, &json).await;
}

async fn respond(stream: &mut TcpStream, status: u16, json: &str) {
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
}

fn error(code: &str) -> String {
    error_with(code, serde_json::json!({}))
}

fn error_with(code: &str, detail: serde_json::Value) -> String {
    serde_json::json!({
        "success": false,
        "error": { "code": code, "message": "mock", "detail": detail },
    })
    .to_string()
}

fn ok(data: serde_json::Value) -> String {
    serde_json::json!({ "success": true, "data": data }).to_string()
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
    content_range: Option<&str>,
) -> (u16, String) {
    let authorised = bearer.is_some_and(|t| state.valid_access.lock().unwrap().contains(t));
    if path.starts_with("/api/v1/uploads") || path == "/api/v1/company-posts" {
        if !authorised {
            return (401, error("invalid_token"));
        }
        return upload_route(state, method, path, body, content_range);
    }

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

fn upload_json(id: &str, u: &MockUpload) -> serde_json::Value {
    let mut v = serde_json::json!({
        "id": id,
        "company_id": u.company_id,
        "status": u.status,
        "total_size": u.total,
        "received_size": u.received,
        "expires_at": "2026-09-16 12:00:00",
    });
    if u.status == "pending" || u.status == "uploading" {
        v["next_offset"] = u.received.into();
    }
    v
}

fn chunk_limit(state: &MockState) -> u64 {
    match state.chunk_bytes.load(Ordering::SeqCst) {
        0 => 5 * 1024 * 1024,
        n => n,
    }
}

fn parse_range(header: Option<&str>) -> Option<(u64, u64, u64)> {
    let rest = header?.trim().strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()?))
}

/// What `append_chunk` leaves behind when the body is cut short.
fn partial_put(
    state: &MockState,
    path: &str,
    bearer: Option<&str>,
    content_range: Option<&str>,
    body: &[u8],
) {
    let authorised = bearer.is_some_and(|t| state.valid_access.lock().unwrap().contains(t));
    let (Some(id), Some((start, _, _))) = (
        path.strip_prefix("/api/v1/uploads/"),
        parse_range(content_range),
    ) else {
        return;
    };
    if !authorised {
        return;
    }
    let mut uploads = state.uploads.lock().unwrap();
    let Some(u) = uploads.get_mut(id) else { return };
    if start == u.received
        && u.file.len() as u64 == u.received
        && state.partial_write_bug.load(Ordering::SeqCst)
    {
        u.file.extend_from_slice(body);
    }
}

fn upload_route(
    state: &MockState,
    method: &str,
    path: &str,
    body: &[u8],
    content_range: Option<&str>,
) -> (u16, String) {
    let json: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();

    match (method, path) {
        ("POST", "/api/v1/uploads") => {
            let n = state.creates.fetch_add(1, Ordering::SeqCst) + 1;
            if json["mime_type"] != "video/mp4" {
                return (415, error("invalid_media_type"));
            }
            let total = json["total_size"].as_u64().unwrap_or(0);
            if total == 0 {
                return (400, error("invalid_request"));
            }
            if total > 52_428_800 {
                return (413, error("upload_too_large"));
            }
            if json["company_id"].as_u64() != Some(1) {
                return (403, error("company_forbidden"));
            }
            let id = format!("{:x}", Sha256::digest(format!("upload-{n}")));
            let upload = MockUpload {
                company_id: 1,
                total,
                file: Vec::new(),
                received: 0,
                status: "pending".into(),
            };
            let mut data = upload_json(&id, &upload);
            data["preferred_chunk_size_bytes"] = chunk_limit(state).into();
            state.uploads.lock().unwrap().insert(id, upload);
            (201, ok(serde_json::json!({ "upload": data })))
        }

        ("GET", p) if p.starts_with("/api/v1/uploads/") => {
            state.status_checks.fetch_add(1, Ordering::SeqCst);
            let id = p.trim_start_matches("/api/v1/uploads/");
            let uploads = state.uploads.lock().unwrap();
            let Some(u) = uploads.get(id) else {
                return (404, error("upload_not_found"));
            };
            (200, ok(serde_json::json!({ "upload": upload_json(id, u) })))
        }

        ("PUT", p) if p.starts_with("/api/v1/uploads/") => {
            let id = p.trim_start_matches("/api/v1/uploads/").to_string();
            let mut uploads = state.uploads.lock().unwrap();
            let Some(u) = uploads.get_mut(&id) else {
                return (404, error("upload_not_found"));
            };
            match u.status.as_str() {
                "completed" => return (409, error("upload_completed")),
                "failed" => return (410, error("upload_failed")),
                _ => {}
            }
            let Some((start, end, total)) = parse_range(content_range) else {
                return (400, error("invalid_content_range"));
            };
            if total != u.total || end < start || end >= total {
                return (400, error("invalid_content_range"));
            }
            let len = end - start + 1;
            if len > chunk_limit(state) {
                return (413, error("chunk_too_large"));
            }
            if body.len() as u64 != len {
                return (400, error("write_incomplete"));
            }
            // The bug's consequence: disk and database disagree, and the session is failed.
            if u.file.len() as u64 != u.received {
                u.status = "failed".into();
                return (500, error("upload_state_mismatch"));
            }
            if start != u.received {
                let at = u.received;
                return (
                    409,
                    error_with(
                        "upload_offset_conflict",
                        serde_json::json!({ "expected_offset": at, "received_size": at }),
                    ),
                );
            }
            u.file.extend_from_slice(body);
            u.received += len;
            u.status = "uploading".into();

            if u.received < u.total {
                return (
                    308,
                    ok(serde_json::json!({ "upload": upload_json(&id, u) })),
                );
            }
            if let Some((status, code)) = state.reject_on_completion.lock().unwrap().clone() {
                u.status = "failed".into();
                return (status, error(&code));
            }
            if state.corrupt_on_completion.load(Ordering::SeqCst) {
                let middle = u.file.len() / 2;
                u.file[middle] ^= 0x01;
            }
            let sha = format!("{:x}", Sha256::digest(&u.file));
            u.status = "completed".into();
            *state.completed_sha256.lock().unwrap() = Some(sha.clone());
            let mut data = upload_json(&id, u);
            data["sha256"] = sha.into();
            data["duration_seconds"] = 12.0.into();
            (200, ok(serde_json::json!({ "upload": data })))
        }

        ("POST", "/api/v1/company-posts") => {
            let Some(upload_id) = json["upload_id"].as_str() else {
                return (400, error("invalid_request"));
            };
            let uploads = state.uploads.lock().unwrap();
            let Some(u) = uploads.get(upload_id) else {
                return (404, error("upload_not_found"));
            };
            if u.status != "completed" {
                return (409, error("upload_not_completed"));
            }
            let mut posts = state.posts.lock().unwrap();
            let (status, id) = match posts.get(upload_id) {
                Some(id) => (200, *id),
                None => {
                    let id = state.posts_created.fetch_add(1, Ordering::SeqCst) as u64 + 1;
                    posts.insert(upload_id.to_string(), id);
                    (201, id)
                }
            };
            let post = serde_json::json!({
                "id": id,
                "slug": "spiral-vazo",
                "company_id": 1,
                "title": json["title"],
                "post_url": "http://localhost/stepperskipcom/c/spiral-vazo",
                "media_url": "http://localhost/stepperskipcom/media/company-video/spiral-vazo",
                "published": true,
                "published_at": "2026-09-15 12:00:00",
            });
            (status, ok(serde_json::json!({ "post": post })))
        }

        _ => (404, error("not_found")),
    }
}
