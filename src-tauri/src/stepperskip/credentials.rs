//! Where the refresh token lives between launches.
//!
//! The operating system's credential store — Windows Credential Manager, the macOS Keychain —
//! and nowhere else. Not `settings.json`, not the app data directory: a refresh token is a
//! ninety-day key to the user's account, and a plain file is readable by anything running as
//! them.
//!
//! The access token is deliberately not stored at all. It lives in memory and is re-derived from
//! the refresh token on the first call after a launch.

#[cfg(test)]
use std::sync::Mutex;

pub trait CredentialStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, String>;
    fn save(&self, refresh_token: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

/// Service name for the credential store entry — the app's bundle identifier.
const SERVICE: &str = "app.skipframe.desktop";

/// The platform credential store.
///
/// The entry is keyed by the StepperSkip base URL, so a development login against localhost and
/// a production login never overwrite each other's token.
pub struct KeyringStore {
    account: String,
}

impl KeyringStore {
    pub fn new(base_url: &str) -> Self {
        KeyringStore {
            account: format!("stepperskip-refresh-token@{base_url}"),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl CredentialStore for KeyringStore {
    fn load(&self) -> Result<Option<String>, String> {
        match entry(&self.account)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, refresh_token: &str) -> Result<(), String> {
        entry(&self.account)?
            .set_password(refresh_token)
            .map_err(|e| e.to_string())
    }

    fn clear(&self) -> Result<(), String> {
        match entry(&self.account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())
}

/// Linux is out of scope for SkipFrame, but the crate should still build there: refuse rather
/// than quietly keep a token in memory and lose it on exit.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl CredentialStore for KeyringStore {
    fn load(&self) -> Result<Option<String>, String> {
        let _ = (&self.account, SERVICE);
        Ok(None)
    }
    fn save(&self, _: &str) -> Result<(), String> {
        Err("no credential store on this platform".into())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
}

/// An in-memory store, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    token: Mutex<Option<String>>,
}

#[cfg(test)]
impl CredentialStore for MemoryStore {
    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.token.lock().unwrap().clone())
    }
    fn save(&self, refresh_token: &str) -> Result<(), String> {
        *self.token.lock().unwrap() = Some(refresh_token.to_string());
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        *self.token.lock().unwrap() = None;
        Ok(())
    }
}
