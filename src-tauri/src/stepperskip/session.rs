//! The signed-in StepperSkip account: sign-in, tokens, sign-out.
//!
//! Two properties of StepperSkip's token design shape everything here.
//!
//! **Refresh tokens rotate, and reuse revokes the whole family.** Using a refresh token hands back
//! a new one and invalidates the old. Presenting the old one again is treated as theft and kills
//! every token in the chain. So a refresh must happen once, not once per caller — three account
//! requests firing together after a launch would otherwise send the same refresh token three
//! times and sign the user out on the second. [`Account::access_token`] holds a lock across the
//! whole refresh, and every caller waits behind it.
//!
//! **The new refresh token must be saved before it is relied on.** If the process died after the
//! server rotated but before the credential store was written, the stored token would be the
//! dead one, and the next launch would trip reuse detection. Saving comes first; if it fails, the
//! session is dropped rather than carried on with a token nothing remembers.

use std::fmt;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::{oneshot, Mutex};

use super::api::{
    self, ApiError, Capabilities, CompaniesData, Company, Http, Limits, LimitsData, MeData, User,
};
use super::callback_page::{self, Outcome};
use super::credentials::CredentialStore;
use super::loopback::{Callback, Loopback};
use super::pkce::{self, Pkce};

/// Refresh this long before expiry rather than find out from a 401 halfway through a request.
const REFRESH_MARGIN: Duration = Duration::from_secs(5 * 60);

/// How long the app waits for the browser. Long enough to sign in, or to reset a forgotten
/// password in the same tab; short enough that an abandoned tab does not hold a port forever.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Revocation is a courtesy to the server. Signing out locally must not wait on it.
const REVOKE_TIMEOUT: Duration = Duration::from_secs(5);

struct AccessToken {
    value: String,
    expires_at: Instant,
}

/// Tokens must never reach a log line, so the one type that holds one refuses to print it.
impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessToken")
            .field("value", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Everything the front end shows about the account, fetched together.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSnapshot {
    pub user: User,
    pub companies: Vec<Company>,
    pub limits: Limits,
    pub capabilities: Capabilities,
}

pub struct Account {
    http: Http,
    store: Arc<dyn CredentialStore>,
    token: Mutex<Option<AccessToken>>,
    /// Present while a sign-in is waiting on the browser; sending on it abandons that wait.
    pending_sign_in: StdMutex<Option<oneshot::Sender<()>>>,
}

impl Account {
    pub fn new(base_url: &str, store: Arc<dyn CredentialStore>) -> Result<Self, ApiError> {
        Ok(Account {
            http: Http::new(base_url)?,
            store,
            token: Mutex::new(None),
            pending_sign_in: StdMutex::new(None),
        })
    }

    pub fn base_url(&self) -> &str {
        self.http.base()
    }

    /// Whether a refresh token is stored. Touches the credential store, never the network.
    pub async fn has_credential(&self) -> Result<bool, ApiError> {
        Ok(self.load().await?.is_some())
    }

    // -- sign-in ------------------------------------------------------------------------------

    /// Run the whole browser sign-in and return the account it produced.
    ///
    /// `open_browser` is given the authorize URL. In the app it hands the URL to the system
    /// browser; in tests it plays the browser.
    pub async fn sign_in<F>(&self, open_browser: F) -> Result<AccountSnapshot, ApiError>
    where
        F: FnOnce(&str) -> Result<(), String>,
    {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        {
            let mut slot = self.pending_sign_in.lock().unwrap();
            if slot.is_some() {
                return Err(ApiError::local(
                    "sign_in_in_progress",
                    "a sign-in is already waiting on the browser",
                ));
            }
            *slot = Some(cancel_tx);
        }
        let _clear = ClearPending(&self.pending_sign_in);

        let pkce = Pkce::generate().map_err(|e| ApiError::local("random", e.to_string()))?;
        let state =
            pkce::random_url_token(32).map_err(|e| ApiError::local("random", e.to_string()))?;
        let loopback = Loopback::bind()
            .await
            .map_err(|e| ApiError::local("loopback", e.to_string()))?;
        let authorize = api::authorize_url(
            self.http.base(),
            loopback.redirect_uri(),
            &state,
            &pkce.challenge,
        )?;

        open_browser(&authorize).map_err(|e| ApiError::local("browser_open_failed", e))?;

        let (callback, reply) = tokio::select! {
            result = loopback.accept_callback() => {
                result.map_err(|e| ApiError::local("loopback", e.to_string()))?
            }
            _ = cancel_rx => {
                return Err(ApiError::local("sign_in_cancelled", "sign-in cancelled"));
            }
            _ = tokio::time::sleep(SIGN_IN_TIMEOUT) => {
                return Err(ApiError::local("sign_in_timeout", "the browser did not come back"));
            }
        };

        let result = self
            .complete_sign_in(callback, &state, &pkce, loopback.redirect_uri())
            .await;

        // Only now does the browser tab find out what happened — after the code was exchanged,
        // the token stored and the account read, not when the code merely arrived.
        let page = match &result {
            Ok(snapshot) => callback_page::render(&Outcome::SignedIn {
                user: &snapshot.user,
                companies: &snapshot.companies,
            }),
            Err(e) => callback_page::render(&Outcome::Failed { code: &e.code }),
        };
        reply.send(&page).await;

        result
    }

    /// Everything between the browser coming back and the account being known.
    async fn complete_sign_in(
        &self,
        callback: Callback,
        state: &str,
        pkce: &Pkce,
        redirect_uri: &str,
    ) -> Result<AccountSnapshot, ApiError> {
        let code = match callback {
            Callback::Code {
                code,
                state: returned,
            } => {
                // Anything but an exact match means this code was not issued for this request.
                // Abort; do not "try it anyway".
                if !constant_time_eq(returned.as_bytes(), state.as_bytes()) {
                    return Err(ApiError::local(
                        "state_mismatch",
                        "the sign-in response did not match the request",
                    ));
                }
                code
            }
            Callback::Error { error, .. } => {
                return Err(ApiError::local(
                    &error,
                    "StepperSkip did not authorise SkipFrame",
                ));
            }
            Callback::Other => unreachable!("accept_callback only returns on a callback"),
        };

        let tokens = self
            .http
            .exchange_code(&code, redirect_uri, &pkce.verifier)
            .await?;

        // The refresh token is persisted before anything uses the session.
        self.save(&tokens.refresh_token).await?;
        *self.token.lock().await = Some(AccessToken {
            value: tokens.access_token,
            expires_at: Instant::now() + Duration::from_secs(tokens.expires_in),
        });

        self.snapshot().await
    }

    /// Abandon a sign-in that is waiting on the browser. Harmless if none is.
    pub fn cancel_sign_in(&self) {
        if let Some(tx) = self.pending_sign_in.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }

    // -- tokens -------------------------------------------------------------------------------

    /// A usable access token, refreshing it if it is missing or about to expire.
    ///
    /// Single-flight: the lock is held for the entire refresh, so concurrent callers wait for
    /// the one refresh instead of each presenting the same rotating refresh token.
    pub async fn access_token(&self) -> Result<String, ApiError> {
        let mut slot = self.token.lock().await;

        if let Some(token) = slot.as_ref() {
            if token.expires_at > Instant::now() + REFRESH_MARGIN {
                return Ok(token.value.clone());
            }
        }

        let Some(refresh_token) = self.load().await? else {
            *slot = None;
            return Err(ApiError::local("not_signed_in", "no StepperSkip session"));
        };

        match self.http.refresh(&refresh_token).await {
            Ok(tokens) => {
                if let Err(e) = self.save(&tokens.refresh_token).await {
                    // The server has already rotated, so the stored token is dead and the new
                    // one cannot be kept. Carrying on would only postpone the failure to the next
                    // launch; end the session now, where it can be explained.
                    *slot = None;
                    let _ = self.clear().await;
                    return Err(e);
                }
                let value = tokens.access_token.clone();
                *slot = Some(AccessToken {
                    value: tokens.access_token,
                    expires_at: Instant::now() + Duration::from_secs(tokens.expires_in),
                });
                Ok(value)
            }
            Err(e) if e.code == "invalid_grant" || e.code == "invalid_client" => {
                // Expired, revoked, or the family was killed by reuse detection. The stored
                // token will never work again; forget it so the interface can ask for a login.
                *slot = None;
                let _ = self.clear().await;
                Err(ApiError {
                    status: e.status,
                    code: "session_expired".into(),
                    message: e.message,
                })
            }
            // Network trouble: keep the refresh token and let the caller try again later.
            Err(e) => Err(e),
        }
    }

    /// Forget an access token the server just rejected — but only if it is still the one we
    /// hold. Another caller may already have refreshed, and throwing away its fresh token would
    /// cost a second, pointless rotation.
    async fn invalidate(&self, rejected: &str) {
        let mut slot = self.token.lock().await;
        if slot.as_ref().is_some_and(|t| t.value == rejected) {
            *slot = None;
        }
    }

    /// GET a resource with the account's token. A 401 earns exactly one refresh and one retry.
    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let token = self.access_token().await?;
        match self.http.get(path, &token).await {
            Err(e) if e.status == 401 => {
                self.invalidate(&token).await;
                let token = self.access_token().await?;
                self.http.get(path, &token).await
            }
            other => other,
        }
    }

    /// The user, their companies and the upload limits, fetched in parallel.
    pub async fn snapshot(&self) -> Result<AccountSnapshot, ApiError> {
        let (me, companies, limits) = tokio::try_join!(
            self.get::<MeData>("/api/v1/me"),
            self.get::<CompaniesData>("/api/v1/companies"),
            self.get::<LimitsData>("/api/v1/limits"),
        )?;
        Ok(AccountSnapshot {
            user: me.user,
            companies: companies.companies,
            limits: limits.limits,
            capabilities: limits.capabilities,
        })
    }

    // -- sign-out ----------------------------------------------------------------------------

    /// Revoke the session on the server if it can be reached, and forget it locally regardless.
    pub async fn sign_out(&self) -> Result<(), ApiError> {
        self.cancel_sign_in();
        let refresh_token = self.load().await.ok().flatten();
        *self.token.lock().await = None;
        if let Some(token) = refresh_token {
            let _ = tokio::time::timeout(REVOKE_TIMEOUT, self.http.revoke(&token)).await;
        }
        self.clear().await
    }

    // -- credential store --------------------------------------------------------------------
    //
    // The platform stores are synchronous and, on macOS, can block on a permission prompt, so
    // they run off the async executor.

    async fn load(&self) -> Result<Option<String>, ApiError> {
        let store = self.store.clone();
        blocking(move || store.load()).await
    }

    async fn save(&self, token: &str) -> Result<(), ApiError> {
        let store = self.store.clone();
        let token = token.to_string();
        blocking(move || store.save(&token)).await
    }

    async fn clear(&self) -> Result<(), ApiError> {
        let store = self.store.clone();
        blocking(move || store.clear()).await
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::local("credential_store", e.to_string()))?
        .map_err(|e| ApiError::local("credential_store", e))
}

/// Clears the pending sign-in slot however `sign_in` exits — success, error, or cancellation.
struct ClearPending<'a>(&'a StdMutex<Option<oneshot::Sender<()>>>);

impl Drop for ClearPending<'_> {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = None;
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::super::credentials::MemoryStore;
    use super::super::testing::{browser_returns, MockServer};
    use super::*;

    async fn account(mock: &MockServer) -> (Arc<Account>, Arc<MemoryStore>) {
        let store = Arc::new(MemoryStore::default());
        let account = Account::new(&mock.base_url(), store.clone()).unwrap();
        (Arc::new(account), store)
    }

    #[tokio::test]
    async fn signs_in_through_the_browser_and_loads_the_account() {
        let mock = MockServer::start().await;
        let (account, store) = account(&mock).await;

        let snapshot = account
            .sign_in(browser_returns(mock.clone(), None))
            .await
            .unwrap();

        assert_eq!(snapshot.user.username, "mert.kaya");
        assert_eq!(snapshot.companies.len(), 1);
        // The tab was answered after the account was known, so it can name it.
        let page = mock.last_page().await;
        assert!(page.contains("Giriş tamamlandı"), "{page}");
        assert!(page.contains("@mert.kaya") && page.contains("Teknovada"));
        assert_eq!(snapshot.limits.max_file_size_bytes, 52_428_800);
        // PKCE was actually checked by the mock: a wrong verifier would have been refused.
        assert_eq!(mock.state.code_exchanges.load(Ordering::SeqCst), 1);
        assert_eq!(store.load().unwrap().as_deref(), Some("rt-0"));
    }

    #[tokio::test]
    async fn a_mismatched_state_aborts_before_the_code_is_exchanged() {
        let mock = MockServer::start().await;
        let (account, store) = account(&mock).await;

        let err = account
            .sign_in(browser_returns(mock.clone(), Some("forged-state")))
            .await
            .unwrap_err();

        assert_eq!(err.code, "state_mismatch");
        assert_eq!(mock.state.code_exchanges.load(Ordering::SeqCst), 0);
        // The browser is told the truth too, not "complete".
        let page = mock.last_page().await;
        assert!(page.contains("Giriş tamamlanmadı"), "{page}");
        assert!(!page.contains("Giriş tamamlandı"));
        assert!(store.load().unwrap().is_none());
    }

    #[tokio::test]
    async fn a_denied_consent_is_reported_by_its_code() {
        let mock = MockServer::start().await;
        let (account, _) = account(&mock).await;
        let err = account
            .sign_in(super::super::testing::browser_denies())
            .await
            .unwrap_err();
        assert_eq!(err.code, "access_denied");
    }

    #[tokio::test]
    async fn cancelling_releases_the_wait_and_a_second_sign_in_is_refused_meanwhile() {
        let mock = MockServer::start().await;
        let (account, _) = account(&mock).await;

        let waiting = {
            let account = account.clone();
            tokio::spawn(async move { account.sign_in(|_| Ok(())).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;

        let second = account.sign_in(|_| Ok(())).await.unwrap_err();
        assert_eq!(second.code, "sign_in_in_progress");

        account.cancel_sign_in();
        let first = waiting.await.unwrap().unwrap_err();
        assert_eq!(first.code, "sign_in_cancelled");

        // The slot is free again.
        let again = {
            let account = account.clone();
            tokio::spawn(async move { account.sign_in(|_| Ok(())).await })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        account.cancel_sign_in();
        assert_eq!(again.await.unwrap().unwrap_err().code, "sign_in_cancelled");
    }

    /// The property reuse detection makes mandatory: eight callers, one expired token, one
    /// refresh.
    #[tokio::test]
    async fn concurrent_callers_share_a_single_refresh() {
        let mock = MockServer::start().await;
        let (account, store) = account(&mock).await;
        store.save("rt-0").unwrap();
        mock.set_refresh_token("rt-0");

        let callers: Vec<_> = (0..8)
            .map(|_| {
                let account = account.clone();
                tokio::spawn(async move { account.access_token().await })
            })
            .collect();
        let mut tokens = Vec::new();
        for c in callers {
            tokens.push(c.await.unwrap().unwrap());
        }

        assert_eq!(mock.state.refreshes.load(Ordering::SeqCst), 1);
        assert!(tokens.iter().all(|t| t == &tokens[0]));
        assert_eq!(mock.state.reuse_detected.load(Ordering::SeqCst), 0);
        // The rotated token was persisted.
        assert_eq!(store.load().unwrap().as_deref(), Some("rt-1"));
    }

    #[tokio::test]
    async fn a_revoked_family_ends_the_session_and_forgets_the_token() {
        let mock = MockServer::start().await;
        let (account, store) = account(&mock).await;
        store.save("rt-dead").unwrap();
        mock.set_refresh_token("rt-live");

        let err = account.access_token().await.unwrap_err();
        assert_eq!(err.code, "session_expired");
        assert!(store.load().unwrap().is_none());
    }

    #[tokio::test]
    async fn a_rejected_access_token_is_refreshed_once_and_the_request_retried() {
        let mock = MockServer::start().await;
        let (account, store) = account(&mock).await;
        store.save("rt-0").unwrap();
        mock.set_refresh_token("rt-0");
        // A token that looks valid locally but the server has revoked.
        *account.token.lock().await = Some(AccessToken {
            value: "at-revoked".into(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        });

        let me: MeData = account.get("/api/v1/me").await.unwrap();
        assert_eq!(me.user.username, "mert.kaya");
        assert_eq!(mock.state.refreshes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn network_failure_keeps_the_refresh_token() {
        let store = Arc::new(MemoryStore::default());
        store.save("rt-0").unwrap();
        // Nothing listens on port 9 of the loopback interface.
        let account = Account::new("http://127.0.0.1:9", store.clone()).unwrap();
        let err = account.access_token().await.unwrap_err();
        assert!(err.code == "network" || err.code == "timeout", "{err:?}");
        assert_eq!(store.load().unwrap().as_deref(), Some("rt-0"));
    }

    #[tokio::test]
    async fn signing_out_revokes_and_clears_even_if_the_server_is_gone() {
        let mock = MockServer::start().await;
        let (account, store) = account(&mock).await;
        store.save("rt-0").unwrap();
        account.sign_out().await.unwrap();
        assert_eq!(mock.state.revocations.load(Ordering::SeqCst), 1);
        assert!(store.load().unwrap().is_none());

        let store = Arc::new(MemoryStore::default());
        store.save("rt-0").unwrap();
        let offline = Account::new("http://127.0.0.1:9", store.clone()).unwrap();
        offline.sign_out().await.unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn debug_output_never_contains_the_token() {
        let token = AccessToken {
            value: "super-secret-access-token".into(),
            expires_at: Instant::now(),
        };
        assert!(!format!("{token:?}").contains("super-secret"));
    }
}
