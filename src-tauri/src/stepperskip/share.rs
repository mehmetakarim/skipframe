//! Uploading a finished video and publishing it to a StepperSkip company profile.
//!
//! The protocol, as StepperSkip implements it: `POST /uploads` opens a session, `PUT
//! /uploads/{id}` sends strictly sequential chunks of at most 5 MiB — 308 for every chunk but the
//! last, whose 200 also completes the upload — `GET /uploads/{id}` says how far the server really
//! got, and `POST /company-posts` publishes a completed upload. Publishing the same upload twice
//! returns the first post rather than making a second, so that request is safe to repeat.
//!
//! Three things this module does on top of the protocol:
//!
//! **The server's progress is the truth.** After anything goes wrong — a connection dropping, a
//! lost answer, a rate limit — the next offset comes from the server, never from what this side
//! believes it sent.
//!
//! **One automatic restart.** Before StepperSkip commit `c182aec`, a chunk whose body arrived cut
//! short left its partial bytes on disk without recording them, and the next chunk failed the
//! whole session with `upload_state_mismatch`. That is fixed in the local install — `append_chunk`
//! now rolls the partial write back — but production has not been verified and may still run the
//! old code. A session that has died is therefore started over once, from zero, before giving up.
//! Against a fixed server this path never runs.
//!
//! **What is published is what was exported.** The file is hashed before a byte is sent, and the
//! SHA-256 the server computes on completion must match before anything is published.

use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::sync::CancellationToken;

use super::api::{ApiError, Post, PostData, Response, UploadData, UploadState};
use super::session::Account;

/// StepperSkip's hard ceiling per chunk. Its preferred size is used when smaller.
const MAX_CHUNK_BYTES: u64 = 5 * 1024 * 1024;

/// A 5 MiB chunk on a slow uplink takes far longer than the 30 seconds a JSON request gets.
const CHUNK_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareRequest {
    pub path: PathBuf,
    pub company_id: u64,
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Reading and hashing the file.
    Preparing,
    Uploading,
    /// Something failed; waiting, then asking the server where it is.
    Reconnecting,
    /// The server lost the session; starting the upload over, once.
    Restarting,
    Publishing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareProgress {
    pub stage: Stage,
    /// Bytes the server has acknowledged — not bytes written to the socket.
    pub sent_bytes: u64,
    pub total_bytes: u64,
}

/// Exponential backoff. StepperSkip sends no `Retry-After` with its 429s, so the client decides.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub base: Duration,
    pub max: Duration,
    /// Consecutive failures without progress before giving up.
    pub attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            base: Duration::from_secs(1),
            max: Duration::from_secs(30),
            attempts: 6,
        }
    }
}

impl RetryPolicy {
    fn delay(&self, failure: u32) -> Duration {
        let factor = 1u32 << failure.saturating_sub(1).min(16);
        self.base.saturating_mul(factor).min(self.max)
    }
}

/// Per-account share state, owned by [`Account`].
#[derive(Default)]
pub struct ShareState {
    cancel: StdMutex<Option<CancellationToken>>,
    /// The last upload that did not end in a post. Sharing the same file to the same company
    /// again continues it instead of opening a new session — StepperSkip allows only 20 new
    /// sessions an hour and 5 unfinished ones at a time.
    unfinished: StdMutex<Option<Unfinished>>,
    pub(super) retry: StdMutex<RetryPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct FileIdentity {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct Unfinished {
    file: FileIdentity,
    company_id: u64,
    upload_id: String,
    chunk_bytes: u64,
}

struct LocalFile {
    identity: FileIdentity,
    name: String,
    sha256: String,
}

impl Account {
    /// Upload the video at `request.path` and publish it. `progress` is called as the upload
    /// moves; in the app it becomes a Tauri event.
    pub async fn share<P>(&self, request: &ShareRequest, progress: P) -> Result<Post, ApiError>
    where
        P: Fn(ShareProgress) + Send + Sync,
    {
        let cancel = CancellationToken::new();
        {
            let mut slot = self.share.cancel.lock().unwrap();
            if slot.is_some() {
                return Err(ApiError::local(
                    "share_in_progress",
                    "a video is already being shared",
                ));
            }
            *slot = Some(cancel.clone());
        }
        let _clear = ClearCancel(&self.share.cancel);

        tokio::select! {
            result = self.run_share(request, &progress) => result,
            // Dropping the upload mid-request is deliberate: waiting for a 5 MiB chunk to finish
            // would make "cancel" take a minute on a slow link. The unfinished session is kept,
            // so sharing the same file again picks it up.
            _ = cancel.cancelled() => Err(ApiError::local("share_cancelled", "sharing cancelled")),
        }
    }

    pub fn cancel_share(&self) {
        if let Some(token) = self.share.cancel.lock().unwrap().as_ref() {
            token.cancel();
        }
    }

    /// Forget the upload that did not finish. Called on sign-out: it belongs to that account.
    pub(super) fn forget_unfinished(&self) {
        *self.share.unfinished.lock().unwrap() = None;
    }

    async fn run_share<P: Fn(ShareProgress)>(
        &self,
        request: &ShareRequest,
        progress: &P,
    ) -> Result<Post, ApiError> {
        let retry = *self.share.retry.lock().unwrap();
        let file = inspect(&request.path).await?;
        let total = file.identity.size;
        progress(ShareProgress {
            stage: Stage::Preparing,
            sent_bytes: 0,
            total_bytes: total,
        });

        let mut restarted = false;
        let completed = loop {
            let (upload, chunk_bytes) = self.resume_or_create(request, &file).await?;
            match self
                .send_chunks(&file, upload, chunk_bytes, retry, progress)
                .await
            {
                Ok(done) => break done,
                Err(e) if !restarted && session_is_dead(&e) => {
                    restarted = true;
                    self.forget_unfinished();
                    progress(ShareProgress {
                        stage: Stage::Restarting,
                        sent_bytes: 0,
                        total_bytes: total,
                    });
                }
                Err(e) => return Err(e),
            }
        };

        let server_hash = completed.sha256.as_deref().map(str::to_ascii_lowercase);
        if server_hash.as_deref() != Some(file.sha256.as_str()) {
            // Do not publish a video that is not the one on disk — whether it changed during the
            // upload or arrived damaged, the post would not be what the user exported.
            self.forget_unfinished();
            return Err(ApiError::local(
                "checksum_mismatch",
                "the uploaded video does not match the file on disk",
            ));
        }

        progress(ShareProgress {
            stage: Stage::Publishing,
            sent_bytes: total,
            total_bytes: total,
        });
        let post = self.publish(request, &completed.id, retry).await?;
        self.forget_unfinished();
        Ok(post)
    }

    /// Continue the previous upload of this file to this company if the server still has it;
    /// otherwise open a new session.
    async fn resume_or_create(
        &self,
        request: &ShareRequest,
        file: &LocalFile,
    ) -> Result<(UploadState, u64), ApiError> {
        let previous = self
            .share
            .unfinished
            .lock()
            .unwrap()
            .clone()
            .filter(|u| u.file == file.identity && u.company_id == request.company_id);

        if let Some(previous) = previous {
            match self.upload_status(&previous.upload_id).await {
                // The server's total must match the file, or the session is not for these bytes.
                Ok(state)
                    if state.total_size == file.identity.size
                        && matches!(
                            state.status.as_str(),
                            "pending" | "uploading" | "completed"
                        ) =>
                {
                    return Ok((state, previous.chunk_bytes));
                }
                // Failed, expired, gone, or for a different size: open a new one.
                Ok(_) => {}
                Err(e) if e.code == "upload_not_found" => {}
                Err(e) => return Err(e),
            }
            self.forget_unfinished();
        }

        let url = self.http.url("/api/v1/uploads");
        // Field names from `Skipframe_api::create_upload` — `filename` and `mime_type`, not the
        // `original_filename` / `declared_mime_type` the handoff gave, which are its database
        // columns.
        let body = serde_json::json!({
            "company_id": request.company_id,
            "total_size": file.identity.size,
            "mime_type": "video/mp4",
            "filename": file.name,
        });
        let created = self
            .authorized::<UploadData, _>(|token| {
                self.http.client().post(&url).bearer_auth(token).json(&body)
            })
            .await?
            .data
            .upload;
        check_upload_id(&created.id)?;

        let chunk_bytes = created
            .preferred_chunk_size_bytes
            .unwrap_or(MAX_CHUNK_BYTES)
            .clamp(1, MAX_CHUNK_BYTES);
        *self.share.unfinished.lock().unwrap() = Some(Unfinished {
            file: file.identity.clone(),
            company_id: request.company_id,
            upload_id: created.id.clone(),
            chunk_bytes,
        });
        Ok((created, chunk_bytes))
    }

    async fn send_chunks<P: Fn(ShareProgress)>(
        &self,
        file: &LocalFile,
        upload: UploadState,
        chunk_bytes: u64,
        retry: RetryPolicy,
        progress: &P,
    ) -> Result<UploadState, ApiError> {
        if upload.status == "completed" {
            return Ok(upload);
        }
        let total = file.identity.size;
        let id = upload.id;
        let mut offset = upload.next_offset.unwrap_or(upload.received_size);
        let mut failures = 0u32;

        loop {
            progress(ShareProgress {
                stage: Stage::Uploading,
                sent_bytes: offset,
                total_bytes: total,
            });

            if offset >= total {
                // Everything is on the server but it has not said "completed" — ask it.
                let state = self.upload_status(&id).await?;
                return match state.status.as_str() {
                    "completed" => Ok(state),
                    status => Err(dead_session(status, None)),
                };
            }

            let end = (offset + chunk_bytes).min(total);
            let bytes = read_range(&file.identity.path, offset, end).await?;

            let outcome = self.put_chunk(&id, offset, end, total, bytes).await;
            match outcome {
                Ok(response) if response.status == 308 => {
                    let state = response.data.upload;
                    offset = state.next_offset.unwrap_or(state.received_size);
                    failures = 0;
                }
                Ok(response) => return Ok(response.data.upload),

                // The server already has more than we sent from — typically because a previous
                // chunk arrived but its answer did not. It says where to continue.
                Err(e) if e.code == "upload_offset_conflict" => {
                    failures += 1;
                    if failures > retry.attempts {
                        return Err(e);
                    }
                    offset = e
                        .detail
                        .get("expected_offset")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| e.clone())?;
                }

                Err(e) if e.code == "upload_completed" || is_transient(&e) => {
                    failures += 1;
                    if failures > retry.attempts {
                        return Err(e);
                    }
                    if e.code != "upload_completed" {
                        progress(ShareProgress {
                            stage: Stage::Reconnecting,
                            sent_bytes: offset,
                            total_bytes: total,
                        });
                        tokio::time::sleep(retry.delay(failures)).await;
                    }
                    match self.upload_status(&id).await {
                        Ok(state) if state.status == "completed" => return Ok(state),
                        Ok(state) if matches!(state.status.as_str(), "pending" | "uploading") => {
                            offset = state.next_offset.unwrap_or(state.received_size);
                        }
                        Ok(state) => return Err(dead_session(&state.status, Some(&e))),
                        // Still unreachable: go round again from the same offset.
                        Err(status_error) if is_transient(&status_error) => {}
                        Err(status_error) => return Err(status_error),
                    }
                }

                Err(e) => return Err(e),
            }
        }
    }

    async fn put_chunk(
        &self,
        id: &str,
        start: u64,
        end: u64,
        total: u64,
        bytes: Bytes,
    ) -> Result<Response<UploadData>, ApiError> {
        let url = self.http.url(&format!("/api/v1/uploads/{id}"));
        let range = format!("bytes {}-{}/{}", start, end - 1, total);
        self.authorized(|token| {
            self.http
                .client()
                .put(&url)
                .bearer_auth(token)
                .header(reqwest::header::CONTENT_RANGE, &range)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .timeout(CHUNK_TIMEOUT)
                .body(bytes.clone())
        })
        .await
    }

    async fn upload_status(&self, id: &str) -> Result<UploadState, ApiError> {
        check_upload_id(id)?;
        Ok(self
            .get::<UploadData>(&format!("/api/v1/uploads/{id}"))
            .await?
            .upload)
    }

    async fn publish(
        &self,
        request: &ShareRequest,
        upload_id: &str,
        retry: RetryPolicy,
    ) -> Result<Post, ApiError> {
        let url = self.http.url("/api/v1/company-posts");
        let body = serde_json::json!({
            "company_id": request.company_id,
            "upload_id": upload_id,
            "title": request.title,
            "description": request.description,
        });
        let mut failures = 0u32;
        loop {
            let outcome = self
                .authorized::<PostData, _>(|token| {
                    self.http.client().post(&url).bearer_auth(token).json(&body)
                })
                .await;
            match outcome {
                Ok(response) => return Ok(response.data.post),
                // Publishing the same upload again returns the post it already made, so an answer
                // that never arrived is safe to ask for twice.
                Err(e) if is_transient(&e) => {
                    failures += 1;
                    if failures > retry.attempts {
                        return Err(e);
                    }
                    tokio::time::sleep(retry.delay(failures)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Worth waiting and trying again. A 503 `media_validation_unavailable` is not: the server
/// cannot validate video at all, and re-sending will not change that.
fn is_transient(e: &ApiError) -> bool {
    matches!(e.code.as_str(), "network" | "timeout" | "rate_limited")
        || (e.status >= 500 && e.code != "media_validation_unavailable")
}

/// The session cannot continue; a new one might.
fn session_is_dead(e: &ApiError) -> bool {
    matches!(
        e.code.as_str(),
        "upload_failed" | "upload_expired" | "upload_not_found"
    )
}

fn dead_session(status: &str, cause: Option<&ApiError>) -> ApiError {
    let code = match status {
        "expired" => "upload_expired",
        _ => "upload_failed",
    };
    let mut error = ApiError::local(code, format!("the upload session is {status}"));
    if let Some(cause) = cause {
        error.message = format!(
            "{} (after {}: {})",
            error.message, cause.code, cause.message
        );
        error.detail = serde_json::json!({ "cause": cause.code });
    }
    error
}

/// The id goes into a URL path; make sure it is the 64 hex characters StepperSkip issues.
fn check_upload_id(id: &str) -> Result<(), ApiError> {
    if id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ApiError::local("bad_response", "malformed upload id"))
    }
}

/// Size, identity and SHA-256 of the file, and a check that it is an MP4 at all.
///
/// The path comes from the front end. Only files that are MP4 by name and by content are read
/// and sent anywhere — the command cannot be used to upload an arbitrary file.
async fn inspect(path: &Path) -> Result<LocalFile, ApiError> {
    let named_mp4 = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
    if !named_mp4 {
        return Err(ApiError::local("not_mp4", "only .mp4 files can be shared"));
    }

    let meta = tokio::fs::metadata(path).await.map_err(file_read)?;
    let mut file = tokio::fs::File::open(path).await.map_err(file_read)?;

    // ISO BMFF: a box size, then `ftyp`. The same check StepperSkip makes, made before uploading
    // instead of after.
    let mut head = [0u8; 12];
    if meta.len() < 12 || file.read_exact(&mut head).await.is_err() || &head[4..8] != b"ftyp" {
        return Err(ApiError::local(
            "not_mp4",
            "the file is not an MP4 container",
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(head);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).await.map_err(file_read)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(LocalFile {
        identity: FileIdentity {
            path: path.to_path_buf(),
            size: meta.len(),
            modified: meta.modified().ok(),
        },
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("skipframe.mp4")
            .to_string(),
        sha256: format!("{:x}", hasher.finalize()),
    })
}

async fn read_range(path: &Path, start: u64, end: u64) -> Result<Bytes, ApiError> {
    let mut file = tokio::fs::File::open(path).await.map_err(file_read)?;
    file.seek(SeekFrom::Start(start)).await.map_err(file_read)?;
    let mut buf = vec![0u8; (end - start) as usize];
    file.read_exact(&mut buf).await.map_err(file_read)?;
    Ok(Bytes::from(buf))
}

fn file_read(e: std::io::Error) -> ApiError {
    ApiError::local("file_read", e.to_string())
}

struct ClearCancel<'a>(&'a StdMutex<Option<CancellationToken>>);

impl Drop for ClearCancel<'_> {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use super::super::credentials::{CredentialStore, MemoryStore};
    use super::super::testing::{Fault, MockServer};
    use super::*;

    const MIB: u64 = 1024 * 1024;

    fn fast() -> RetryPolicy {
        RetryPolicy {
            base: Duration::from_millis(2),
            max: Duration::from_millis(10),
            attempts: 6,
        }
    }

    async fn signed_in(mock: &MockServer) -> Arc<Account> {
        let store = Arc::new(MemoryStore::default());
        store.save("rt-0").unwrap();
        mock.set_refresh_token("rt-0");
        let account = Account::new(&mock.base_url(), store).unwrap();
        *account.share.retry.lock().unwrap() = fast();
        Arc::new(account)
    }

    /// A file that passes the MP4 check, with content that makes a misplaced byte show up in the
    /// hash.
    fn video(name: &str, size: u64) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sf-share-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut data = Vec::with_capacity(size as usize);
        data.extend_from_slice(&[0, 0, 0, 24]);
        data.extend_from_slice(b"ftypisom");
        let mut x: u32 = 0x9e37_79b9;
        while (data.len() as u64) < size {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            data.push((x >> 11) as u8);
        }
        data.truncate(size as usize);
        std::fs::write(&path, data).unwrap();
        path
    }

    fn sha(path: &Path) -> String {
        format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
    }

    fn request(path: &Path) -> ShareRequest {
        ShareRequest {
            path: path.to_path_buf(),
            company_id: 1,
            title: "Spiral vazo · 842 katman".into(),
            description: "PLA".into(),
        }
    }

    type Log = Arc<Mutex<Vec<ShareProgress>>>;

    fn recorder() -> (Log, impl Fn(ShareProgress) + Send + Sync) {
        let log: Log = Arc::default();
        let sink = log.clone();
        (log, move |p| sink.lock().unwrap().push(p))
    }

    fn stages(log: &Log) -> Vec<Stage> {
        log.lock().unwrap().iter().map(|p| p.stage).collect()
    }

    #[tokio::test]
    async fn uploads_in_order_verifies_and_publishes() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        let account = signed_in(&mock).await;
        let file = video("plain.mp4", 3 * MIB + 512 * 1024);
        let (log, progress) = recorder();

        let post = account.share(&request(&file), progress).await.unwrap();

        assert_eq!(
            post.post_url,
            "http://localhost/stepperskipcom/c/spiral-vazo"
        );
        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1);
        assert_eq!(mock.state.puts.load(Ordering::SeqCst), 4);
        assert_eq!(mock.state.posts_created.load(Ordering::SeqCst), 1);
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));

        let log_stages = stages(&log);
        assert_eq!(log_stages.first(), Some(&Stage::Preparing));
        assert_eq!(log_stages.last(), Some(&Stage::Publishing));
        let sent: Vec<u64> = log
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.stage == Stage::Uploading)
            .map(|p| p.sent_bytes)
            .collect();
        assert_eq!(sent, vec![0, MIB, 2 * MIB, 3 * MIB]);
    }

    #[tokio::test]
    async fn a_lost_answer_resumes_from_what_the_server_has() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.put_fault(2, Fault::LoseResponse);
        let account = signed_in(&mock).await;
        let file = video("lost-answer.mp4", 4 * MIB);
        let (log, progress) = recorder();

        account.share(&request(&file), progress).await.unwrap();

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1, "no restart");
        assert!(mock.state.status_checks.load(Ordering::SeqCst) >= 1);
        // Chunk two reached the server even though its answer did not, so it was not sent again:
        // four chunks, four PUTs. Five would mean the client trusted itself over the server.
        assert_eq!(mock.state.puts.load(Ordering::SeqCst), 4);
        assert!(stages(&log).contains(&Stage::Reconnecting));
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
    }

    #[tokio::test]
    async fn a_dropped_request_is_sent_again_from_the_same_offset() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.put_fault(3, Fault::DropBeforeBody);
        let account = signed_in(&mock).await;
        let file = video("dropped.mp4", 4 * MIB);

        account.share(&request(&file), |_| {}).await.unwrap();

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1);
        assert_eq!(mock.state.puts.load(Ordering::SeqCst), 5);
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
    }

    /// StepperSkip before `c182aec`: a chunk cut short poisons the session, and the one restart
    /// saves the share. Kept because production may still run that code.
    #[tokio::test]
    async fn a_cut_chunk_on_a_server_without_the_fix_restarts_once() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.set_partial_write_bug(true);
        mock.put_fault(2, Fault::CutBody { keep: 300_000 });
        let account = signed_in(&mock).await;
        let file = video("cut-buggy.mp4", 4 * MIB);
        let (log, progress) = recorder();

        account.share(&request(&file), progress).await.unwrap();

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 2, "one restart");
        assert!(stages(&log).contains(&Stage::Restarting));
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
        assert_eq!(mock.state.posts_created.load(Ordering::SeqCst), 1);
    }

    /// The same cut on a server that rolls partial chunks back — StepperSkip from `c182aec` on:
    /// a plain resume, no restart.
    #[tokio::test]
    async fn a_cut_chunk_on_a_fixed_server_just_resumes() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.set_partial_write_bug(false);
        mock.put_fault(2, Fault::CutBody { keep: 300_000 });
        let account = signed_in(&mock).await;
        let file = video("cut-fixed.mp4", 4 * MIB);
        let (log, progress) = recorder();

        account.share(&request(&file), progress).await.unwrap();

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1);
        assert!(!stages(&log).contains(&Stage::Restarting));
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
    }

    #[tokio::test]
    async fn rate_limits_are_waited_out() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.put_fault(1, Fault::RateLimit);
        mock.put_fault(2, Fault::RateLimit);
        let account = signed_in(&mock).await;
        let file = video("rate.mp4", 2 * MIB);

        account.share(&request(&file), |_| {}).await.unwrap();

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1);
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
    }

    #[tokio::test]
    async fn a_lost_publish_answer_does_not_make_a_second_post() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.publish_fault(1, Fault::LoseResponse);
        let account = signed_in(&mock).await;
        let file = video("publish.mp4", MIB);

        let post = account.share(&request(&file), |_| {}).await.unwrap();

        assert_eq!(mock.state.publishes.load(Ordering::SeqCst), 2);
        assert_eq!(mock.state.posts_created.load(Ordering::SeqCst), 1);
        assert_eq!(
            post.post_url,
            "http://localhost/stepperskipcom/c/spiral-vazo"
        );
    }

    #[tokio::test]
    async fn a_token_revoked_mid_upload_is_refreshed_and_the_upload_continues() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.put_fault(2, Fault::RevokeTokens);
        let account = signed_in(&mock).await;
        let file = video("revoked.mp4", 3 * MIB);

        account.share(&request(&file), |_| {}).await.unwrap();

        assert_eq!(mock.state.refreshes.load(Ordering::SeqCst), 2);
        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1);
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
    }

    #[tokio::test]
    async fn a_rejection_from_the_server_is_final() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.reject_on_completion(422, "video_too_long");
        let account = signed_in(&mock).await;
        let file = video("too-long.mp4", 2 * MIB);

        let err = account.share(&request(&file), |_| {}).await.unwrap_err();

        assert_eq!(err.code, "video_too_long");
        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1, "no restart");
        assert_eq!(mock.state.publishes.load(Ordering::SeqCst), 0);
    }

    /// One flipped bit in what the server assembled, and nothing is published.
    #[tokio::test]
    async fn a_damaged_upload_is_never_published() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.corrupt_on_completion();
        let account = signed_in(&mock).await;
        let file = video("damaged.mp4", 2 * MIB);

        let err = account.share(&request(&file), |_| {}).await.unwrap_err();

        assert_eq!(err.code, "checksum_mismatch");
        assert_ne!(mock.completed_sha256(), Some(sha(&file)));
        assert_eq!(mock.state.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn only_mp4_files_are_read_and_nothing_else_reaches_the_server() {
        let mock = MockServer::start().await;
        let account = signed_in(&mock).await;

        let fake = std::env::temp_dir().join(format!("sf-share-{}-fake.mp4", std::process::id()));
        std::fs::write(&fake, b"definitely not an mp4 container, just text").unwrap();
        let err = account.share(&request(&fake), |_| {}).await.unwrap_err();
        assert_eq!(err.code, "not_mp4");

        let renamed = video("clip.mov", MIB);
        let err = account.share(&request(&renamed), |_| {}).await.unwrap_err();
        assert_eq!(err.code, "not_mp4");

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelling_keeps_the_session_and_sharing_again_continues_it() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(MIB);
        mock.set_partial_write_bug(false);
        let account = signed_in(&mock).await;
        let file = video("cancel.mp4", 4 * MIB);

        let canceller = account.clone();
        let err = account
            .share(&request(&file), move |p| {
                if p.stage == Stage::Uploading && p.sent_bytes >= 2 * MIB {
                    canceller.cancel_share();
                }
            })
            .await
            .unwrap_err();
        assert_eq!(err.code, "share_cancelled");
        assert_eq!(mock.state.posts_created.load(Ordering::SeqCst), 0);

        let (log, progress) = recorder();
        account.share(&request(&file), progress).await.unwrap();

        assert_eq!(
            mock.state.creates.load(Ordering::SeqCst),
            1,
            "the same session"
        );
        let first_upload = log
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.stage == Stage::Uploading)
            .map(|p| p.sent_bytes);
        assert!(
            first_upload >= Some(2 * MIB),
            "continued, did not start over: {first_upload:?}"
        );
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
    }

    /// The headline: the largest file StepperSkip accepts, 5 MiB chunks as it prefers, and three
    /// different failures along the way.
    #[tokio::test]
    async fn fifty_mebibytes_through_three_failures() {
        let mock = MockServer::start().await;
        mock.set_chunk_bytes(5 * MIB);
        mock.put_fault(3, Fault::LoseResponse);
        mock.put_fault(6, Fault::DropBeforeBody);
        mock.put_fault(8, Fault::RateLimit);
        let account = signed_in(&mock).await;
        let file = video("fifty.mp4", 50 * MIB);
        let (log, progress) = recorder();

        let started = Instant::now();
        account.share(&request(&file), progress).await.unwrap();
        let took = started.elapsed();

        assert_eq!(mock.state.creates.load(Ordering::SeqCst), 1);
        assert_eq!(mock.state.posts_created.load(Ordering::SeqCst), 1);
        assert_eq!(mock.completed_sha256(), Some(sha(&file)));
        let reconnects = stages(&log)
            .iter()
            .filter(|s| **s == Stage::Reconnecting)
            .count();
        println!(
            "50 MiB: {} PUTs, {} status checks, {} reconnects, {:.2} s",
            mock.state.puts.load(Ordering::SeqCst),
            mock.state.status_checks.load(Ordering::SeqCst),
            reconnects,
            took.as_secs_f64()
        );
    }

    #[test]
    fn backoff_doubles_and_stops_at_the_ceiling() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay(1), Duration::from_secs(1));
        assert_eq!(p.delay(2), Duration::from_secs(2));
        assert_eq!(p.delay(5), Duration::from_secs(16));
        assert_eq!(p.delay(6), Duration::from_secs(30));
        assert_eq!(p.delay(40), Duration::from_secs(30));
    }
}
