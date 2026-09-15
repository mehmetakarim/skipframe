//! PKCE (RFC 7636) and the random values the sign-in flow needs.
//!
//! SkipFrame is an open-source public client with no secret, so the only thing that binds an
//! authorization code to the process that asked for it is the verifier generated here. It never
//! leaves this process except once, in the body of the token exchange.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Result<Self, getrandom::Error> {
        // 32 bytes is the RFC's own recommendation: 43 characters once encoded, the shortest
        // verifier the spec allows and the shortest challenge StepperSkip accepts.
        let verifier = random_url_token(32)?;
        let challenge = challenge_for(&verifier);
        Ok(Pkce {
            verifier,
            challenge,
        })
    }
}

/// `BASE64URL(SHA256(verifier))`, unpadded — the `S256` method.
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Cryptographically random bytes, base64url without padding. Used for the verifier and for
/// `state`.
pub fn random_url_token(bytes: usize) -> Result<String, getrandom::Error> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf)?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worked example in RFC 7636, Appendix B.
    ///
    /// The expected value was also produced by running StepperSkip's own expression from
    /// `Oauth_service::verify_pkce` — `rtrim(strtr(base64_encode(hash('sha256', $v, true)),
    /// '+/', '-_'), '=')` — in the PHP that serves it, so this pins agreement with the server,
    /// not just with the spec.
    #[test]
    fn matches_the_rfc_example() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_is_within_the_spec_and_the_server_limits() {
        let pkce = Pkce::generate().unwrap();
        // RFC 7636: 43..=128 unreserved characters. StepperSkip checks the challenge the same.
        assert_eq!(pkce.verifier.len(), 43);
        assert_eq!(pkce.challenge.len(), 43);
        assert!(pkce
            .verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn two_verifiers_are_never_the_same() {
        let a = Pkce::generate().unwrap();
        let b = Pkce::generate().unwrap();
        assert_ne!(a.verifier, b.verifier);
    }
}
