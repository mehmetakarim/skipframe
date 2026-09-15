//! HTTP to StepperSkip.
//!
//! Every shape here was read from the StepperSkip source (`Skipframe_api.php`, `Oauth.php`,
//! `MY_Controller.php`), not from the written handoff, which got several of them wrong: the
//! resources are wrapped in a named object (`data.user`, `data.companies`), `/limits` carries a
//! separate `capabilities` object, and error codes differ from the handoff's list.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Public client: this id is in the open-source repository and is not a secret. There is no
/// client secret, by design — PKCE takes its place.
pub const CLIENT_ID: &str = "skipframe-desktop";

/// Exactly the three scopes StepperSkip's `skipframe-desktop` client is allowed.
pub const SCOPES: &str = "profile:read company:read video:write";

/// A failure, from the server or from getting to it.
///
/// `code` is what the interface translates. Server codes pass through unchanged; failures that
/// never reached the server (`network`, `bad_response`, ...) get codes of our own. `status` is 0
/// for those.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn local(code: &str, message: impl Into<String>) -> Self {
        ApiError {
            status: 0,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

// -- response shapes ----------------------------------------------------------------------------
//
// Deserialised from StepperSkip's snake_case, serialised to the front end as camelCase — the
// same convention `meta` uses.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct User {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub profile_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Company {
    pub id: u64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub profile_url: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub can_publish: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Limits {
    pub max_file_size_bytes: u64,
    pub max_video_duration_seconds: f64,
    pub preferred_chunk_size_bytes: u64,
    #[serde(default)]
    pub allowed_mime_types: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Capabilities {
    #[serde(default)]
    pub resumable_upload: bool,
    #[serde(default)]
    pub company_video_publish: bool,
}

#[derive(Debug, Deserialize)]
pub struct MeData {
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct CompaniesData {
    pub companies: Vec<Company>,
}

#[derive(Debug, Deserialize)]
pub struct LimitsData {
    pub limits: Limits,
    #[serde(default)]
    pub capabilities: Capabilities,
}

/// `POST /oauth/token`. Unlike every other endpoint this one answers success in plain RFC 6749
/// form, not inside the `{success, data}` envelope — but its errors do use the envelope.
#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: String,
}

/// Written by hand so that a `{:?}` — in a panic message, a test failure, a stray log line —
/// cannot print either token.
impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"<redacted>")
            .field("expires_in", &self.expires_in)
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

// -- decoding ----------------------------------------------------------------------------------

/// Unwrap `{"success": true, "data": ...}`, or turn anything else into an [`ApiError`].
pub fn decode_envelope<T: DeserializeOwned>(status: u16, body: &[u8]) -> Result<T, ApiError> {
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| non_json(status))?;

    if let Some(error) = error_in(status, &value) {
        return Err(error);
    }

    // 308 is not a failure here: it is how an intermediate upload chunk is acknowledged.
    let ok_status = (200..300).contains(&status) || status == 308;
    if ok_status && value.get("success").and_then(|v| v.as_bool()) == Some(true) {
        if let Some(data) = value.get("data") {
            return T::deserialize(data).map_err(|e| {
                ApiError::local("bad_response", format!("unexpected response shape: {e}"))
            });
        }
    }
    Err(ApiError {
        status,
        code: "bad_response".into(),
        message: format!("HTTP {status} without a recognisable body"),
    })
}

/// Decode the token endpoint's answer.
pub fn decode_token(status: u16, body: &[u8]) -> Result<TokenResponse, ApiError> {
    if (200..300).contains(&status) {
        return serde_json::from_slice(body)
            .map_err(|e| ApiError::local("bad_response", format!("token response: {e}")));
    }
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| non_json(status))?;
    Err(error_in(status, &value).unwrap_or_else(|| non_json(status)))
}

/// The error inside a body, in StepperSkip's envelope or — defensively — plain RFC 6749 form.
fn error_in(status: u16, value: &serde_json::Value) -> Option<ApiError> {
    let error = value.get("error")?;
    if let Some(code) = error.get("code").and_then(|c| c.as_str()) {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_default();
        return Some(ApiError {
            status,
            code: code.to_string(),
            message: message.to_string(),
        });
    }
    let code = error.as_str()?;
    let message = value
        .get("error_description")
        .and_then(|m| m.as_str())
        .unwrap_or_default();
    Some(ApiError {
        status,
        code: code.to_string(),
        message: message.to_string(),
    })
}

/// An Apache error page, a PHP fatal, a captive portal — anything that is not our JSON.
fn non_json(status: u16) -> ApiError {
    ApiError {
        status,
        code: "bad_response".into(),
        message: format!("HTTP {status} with a body that is not JSON"),
    }
}

// -- transport ---------------------------------------------------------------------------------

pub struct Http {
    client: reqwest::Client,
    base: String,
}

impl Http {
    pub fn new(base: &str) -> Result<Self, ApiError> {
        // reqwest is built without a bundled crypto provider, matching the updater; install
        // `ring` process-wide. A second install is a harmless no-op.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let client = reqwest::Client::builder()
            .user_agent(format!(
                "SkipFrame/{} ({})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS
            ))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            // Never follow redirects. The API has no reason to send one, and the upload
            // protocol answers intermediate chunks with 308 — a status an HTTP client would
            // otherwise treat as "follow this".
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ApiError::local("network", e.to_string()))?;

        Ok(Http {
            client,
            base: base.trim_end_matches('/').to_string(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<TokenResponse, ApiError> {
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ];
        self.post_token(&form).await
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, ApiError> {
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ];
        self.post_token(&form).await
    }

    async fn post_token(&self, form: &[(&str, &str)]) -> Result<TokenResponse, ApiError> {
        let response = self
            .client
            .post(self.url("/oauth/token"))
            .form(form)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(transport)?;
        decode_token(status, &body)
    }

    /// RFC 7009 revocation. StepperSkip answers 200 whether or not the token existed.
    pub async fn revoke(&self, token: &str) -> Result<(), ApiError> {
        self.client
            .post(self.url("/oauth/revoke"))
            .form(&[("token", token), ("client_id", CLIENT_ID)])
            .send()
            .await
            .map_err(transport)?;
        Ok(())
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        access_token: &str,
    ) -> Result<T, ApiError> {
        let response = self
            .client
            .get(self.url(path))
            // The only place a token is ever sent. StepperSkip accepts it nowhere else — not in
            // the query string, not in a cookie — and neither would we.
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(transport)?;
        decode_envelope(status, &body)
    }
}

/// Could not reach the server, or the connection broke. reqwest's message names the URL, which
/// never carries a token — they travel only in headers and form bodies.
fn transport(e: reqwest::Error) -> ApiError {
    let code = if e.is_timeout() { "timeout" } else { "network" };
    ApiError::local(code, e.to_string())
}

/// The URL the system browser is sent to.
pub fn authorize_url(
    base: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> Result<String, ApiError> {
    let mut url = url::Url::parse(&format!("{}/oauth/authorize", base.trim_end_matches('/')))
        .map_err(|e| ApiError::local("unconfigured", format!("bad StepperSkip URL: {e}")))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", SCOPES)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim from `Skipframe_api.php::me()` as it serialises.
    const ME: &str = r#"{"success":true,"data":{"user":{"id":97,"username":"mert.kaya",
        "display_name":"Mert Kaya","avatar_url":"http://localhost/stepperskipcom/uploads/avatars/default.png",
        "profile_url":"http://localhost/stepperskipcom/@mert.kaya"}}}"#;

    #[test]
    fn unwraps_the_named_resource() {
        let me: MeData = decode_envelope(200, ME.as_bytes()).unwrap();
        assert_eq!(me.user.username, "mert.kaya");
        assert_eq!(me.user.display_name, "Mert Kaya");
    }

    #[test]
    fn serialises_camel_case_for_the_front_end() {
        let me: MeData = decode_envelope(200, ME.as_bytes()).unwrap();
        let out = serde_json::to_value(&me.user).unwrap();
        assert!(out.get("displayName").is_some());
        assert!(out.get("display_name").is_none());
    }

    #[test]
    fn reads_limits_and_capabilities() {
        let body = r#"{"success":true,"data":{"limits":{"max_file_size_bytes":52428800,
            "max_video_duration_seconds":60,"preferred_chunk_size_bytes":5242880,
            "allowed_mime_types":["video/mp4"]},"capabilities":{"resumable_upload":true,
            "company_video_publish":true}}}"#;
        let got: LimitsData = decode_envelope(200, body.as_bytes()).unwrap();
        assert_eq!(got.limits.max_file_size_bytes, 52_428_800);
        assert_eq!(got.limits.max_video_duration_seconds, 60.0);
        assert!(got.capabilities.company_video_publish);
    }

    #[test]
    fn an_empty_company_list_is_a_list() {
        let body = r#"{"success":true,"data":{"companies":[]}}"#;
        let got: CompaniesData = decode_envelope(200, body.as_bytes()).unwrap();
        assert!(got.companies.is_empty());
    }

    /// Verbatim from `MY_Controller::oauth_error` — including `detail` as an empty object.
    #[test]
    fn surfaces_the_error_code_not_the_prose() {
        let body = r#"{"success":false,"error":{"code":"invalid_token",
            "message":"Geçerli bir Bearer belirteci bulunamadı.","detail":{}}}"#;
        let err = decode_envelope::<MeData>(401, body.as_bytes()).unwrap_err();
        assert_eq!(err.status, 401);
        assert_eq!(err.code, "invalid_token");
    }

    #[test]
    fn an_html_error_page_is_a_bad_response_not_a_crash() {
        let err =
            decode_envelope::<MeData>(500, b"<html><body>Internal Server Error</body></html>")
                .unwrap_err();
        assert_eq!(err.code, "bad_response");
        assert_eq!(err.status, 500);
    }

    #[test]
    fn a_successful_body_of_the_wrong_shape_is_a_bad_response() {
        let err =
            decode_envelope::<MeData>(200, br#"{"success":true,"data":{"nope":1}}"#).unwrap_err();
        assert_eq!(err.code, "bad_response");
    }

    #[test]
    fn token_success_is_plain_and_token_errors_are_enveloped() {
        let ok = br#"{"access_token":"a","token_type":"Bearer","expires_in":604800,
            "refresh_token":"r","scope":"profile:read company:read video:write"}"#;
        let t = decode_token(200, ok).unwrap();
        assert_eq!(t.expires_in, 604_800);

        let bad =
            br#"{"success":false,"error":{"code":"invalid_grant","message":"x","detail":{}}}"#;
        assert_eq!(decode_token(400, bad).unwrap_err().code, "invalid_grant");

        // RFC 6749 form, in case a proxy or a future server version answers that way.
        let rfc = br#"{"error":"invalid_grant","error_description":"expired"}"#;
        assert_eq!(decode_token(400, rfc).unwrap_err().code, "invalid_grant");
    }

    #[test]
    fn the_authorize_url_carries_everything_the_server_checks() {
        let url = authorize_url(
            "http://localhost/stepperskipcom/",
            "http://127.0.0.1:53124/callback",
            "st",
            "ch",
        )
        .unwrap();
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(parsed.path(), "/stepperskipcom/oauth/authorize");
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], "skipframe-desktop");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:53124/callback");
        assert_eq!(q["scope"], "profile:read company:read video:write");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "st");
    }

    /// Against the real StepperSkip, not the mock. Ignored by default because it needs the local
    /// XAMPP install; run it with
    /// `cargo test -p skipframe live_contract -- --ignored`.
    ///
    /// It sends no credentials. It checks that the server accepts this client, these scopes and
    /// this PKCE request — an unauthenticated browser is sent to the login page, where a wrong
    /// client id or scope would instead come straight back to the loopback with `?error=` — and
    /// that the resource API answers in the envelope `decode_envelope` expects.
    #[tokio::test]
    #[ignore = "needs the local StepperSkip at http://localhost/stepperskipcom"]
    async fn live_contract() {
        let base = std::env::var("SKIPFRAME_STEPPERSKIP_URL")
            .unwrap_or_else(|_| "http://localhost/stepperskipcom".into());
        let http = Http::new(&base).unwrap();
        let pkce = super::super::pkce::Pkce::generate().unwrap();
        let url = authorize_url(
            &base,
            "http://127.0.0.1:53124/callback",
            "live-contract-state",
            &pkce.challenge,
        )
        .unwrap();

        let response = http.client.get(&url).send().await.unwrap();
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get("location")
            .and_then(|l| l.to_str().ok())
            .unwrap_or_default()
            .to_string();
        println!("authorize -> {status} {location}");
        assert!(
            (300..400).contains(&status),
            "expected a redirect, got {status}"
        );
        assert!(
            !location.starts_with("http://127.0.0.1:53124/callback"),
            "the server bounced the request back with an error: {location}"
        );
        assert!(
            location.contains("giris"),
            "expected the login page: {location}"
        );

        let err = http
            .get::<LimitsData>("/api/v1/limits", "not-a-real-token")
            .await
            .unwrap_err();
        println!("limits with a bogus token -> {} {}", err.status, err.code);
        assert_eq!(err.status, 401);
        assert_eq!(err.code, "invalid_token");
    }
}
