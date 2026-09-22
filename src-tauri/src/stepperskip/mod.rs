//! The StepperSkip account: signing in with the system browser, and the account APIs.
//!
//! Sharing a finished video to a StepperSkip company profile is the only thing in SkipFrame
//! that ever talks to a server, and it only happens when the user asks. G-code never leaves the
//! machine under any path through this module.
//!
//! The contract was verified against StepperSkip's own source rather than taken from its written
//! handoff; see `api.rs` for what differed.

mod api;
mod callback_page;
mod credentials;
mod loopback;
mod pkce;
mod session;
mod share;
#[cfg(test)]
mod testing;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::IpcError;
use api::ApiError;
use api::Post;
use credentials::KeyringStore;
use session::{Account, AccountSnapshot};
use share::ShareRequest;

/// Emitted as a shared video uploads and publishes; the payload is `share::ShareProgress`.
const SHARE_PROGRESS: &str = "skipframe://share-progress";

/// Where StepperSkip is.
///
/// In order: `SKIPFRAME_STEPPERSKIP_URL` at run time, the same variable at build time, and in a
/// debug build only, the local XAMPP install. The release workflow sets the build-time variable
/// to production; a release built without it has no StepperSkip at all rather than pointing at
/// a server nobody has verified.
fn base_url() -> Option<String> {
    let chosen = std::env::var("SKIPFRAME_STEPPERSKIP_URL")
        .ok()
        .or_else(|| option_env!("SKIPFRAME_STEPPERSKIP_URL").map(str::to_string))
        .or_else(|| {
            cfg!(debug_assertions).then(|| "http://localhost/stepperskipcom".to_string())
        })?;
    let trimmed = chosen.trim().trim_end_matches('/').to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Managed state. `account` is `None` when this build has no StepperSkip to talk to.
pub struct StepperSkip {
    account: Option<Account>,
}

impl StepperSkip {
    pub fn from_environment() -> Self {
        let account = base_url().and_then(|base| {
            let store = Arc::new(KeyringStore::new(&base));
            Account::new(&base, store).ok()
        });
        StepperSkip { account }
    }

    fn account(&self) -> Result<&Account, IpcError> {
        self.account.as_ref().ok_or_else(|| {
            IpcError::new(
                "stepperskip_unconfigured",
                "this build has no StepperSkip server configured",
            )
        })
    }
}

impl From<ApiError> for IpcError {
    fn from(e: ApiError) -> Self {
        IpcError {
            code: e.code,
            message: e.message,
            detail: e.detail,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    configured: bool,
    base_url: Option<String>,
    has_credential: bool,
}

/// Whether sharing is available in this build, and whether a session is stored. No network.
#[tauri::command]
pub async fn ss_status(state: State<'_, StepperSkip>) -> Result<Status, IpcError> {
    let Some(account) = state.account.as_ref() else {
        return Ok(Status {
            configured: false,
            base_url: None,
            has_credential: false,
        });
    };
    Ok(Status {
        configured: true,
        base_url: Some(account.base_url().to_string()),
        has_credential: account.has_credential().await?,
    })
}

/// Open StepperSkip in the system browser and wait for the user to come back signed in.
#[tauri::command]
pub async fn ss_sign_in(
    app: AppHandle,
    state: State<'_, StepperSkip>,
) -> Result<AccountSnapshot, IpcError> {
    let account = state.account()?;
    let snapshot = account
        .sign_in(|url| {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        })
        .await?;
    Ok(snapshot)
}

#[tauri::command]
pub fn ss_cancel_sign_in(state: State<'_, StepperSkip>) {
    if let Some(account) = state.account.as_ref() {
        account.cancel_sign_in();
    }
}

/// The stored session's user, companies and limits. Refreshes the access token if it needs to.
#[tauri::command]
pub async fn ss_account(state: State<'_, StepperSkip>) -> Result<AccountSnapshot, IpcError> {
    Ok(state.account()?.snapshot().await?)
}

#[tauri::command]
pub async fn ss_sign_out(state: State<'_, StepperSkip>) -> Result<(), IpcError> {
    Ok(state.account()?.sign_out().await?)
}

/// Upload a finished MP4 and publish it to one of the account's company profiles. Progress
/// arrives as `skipframe://share-progress` events; the result is the published post.
#[tauri::command]
pub async fn ss_share(
    app: AppHandle,
    state: State<'_, StepperSkip>,
    request: ShareRequest,
) -> Result<Post, IpcError> {
    let account = state.account()?;
    let post = account
        .share(&request, |progress| {
            let _ = app.emit(SHARE_PROGRESS, progress);
        })
        .await?;
    Ok(post)
}

#[tauri::command]
pub fn ss_cancel_share(state: State<'_, StepperSkip>) {
    if let Some(account) = state.account.as_ref() {
        account.cancel_share();
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Page {
    /// Where a user without a company profile goes to open one.
    CompanySetup,
    /// A URL StepperSkip itself returned — a profile or a company page.
    Url(String),
}

/// Open a StepperSkip page in the system browser.
///
/// Only pages on the configured StepperSkip origin: the front end cannot use this command to
/// open an arbitrary address.
#[tauri::command]
pub fn ss_open_page(
    app: AppHandle,
    state: State<'_, StepperSkip>,
    page: Page,
) -> Result<(), IpcError> {
    let base = state.account()?.base_url().to_string();
    let url = match page {
        Page::CompanySetup => format!("{base}/profil/company"),
        Page::Url(url) if url.starts_with(&format!("{base}/")) => url,
        Page::Url(_) => return Err(IpcError::new("invalid_request", "not a StepperSkip page")),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| IpcError::new("browser_open_failed", e.to_string()))
}
