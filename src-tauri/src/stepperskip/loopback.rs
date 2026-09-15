//! The one-request HTTP listener the browser comes back to after sign-in (RFC 8252 §7.3).
//!
//! It binds `127.0.0.1` on a port the OS picks, answers exactly one `GET /callback`, and is
//! dropped. A full HTTP server for a single redirect would be a dependency with a much larger
//! surface than the thing it does, so this reads one request line by hand.
//!
//! Only the loopback interface is bound. Nothing on the network can reach the port, and
//! StepperSkip refuses any redirect that is not `127.0.0.1` or `::1` with the path `/callback`.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The most of a request we will read. A callback URL is a few hundred bytes; anything past this
/// is not our browser.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

pub struct Loopback {
    listener: TcpListener,
    redirect_uri: String,
}

/// What the browser brought back.
#[derive(Debug, PartialEq, Eq)]
pub enum Callback {
    /// The user approved. `state` still has to be compared by the caller.
    Code { code: String, state: String },
    /// StepperSkip redirected with `?error=` — denied, bad client, bad scope.
    Error {
        error: String,
        state: Option<String>,
    },
    /// Something else asked the port for something — a favicon, typically. Answer 404 and keep
    /// waiting.
    Other,
}

impl Loopback {
    pub async fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        Ok(Loopback {
            listener,
            redirect_uri: format!("http://127.0.0.1:{port}/callback"),
        })
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Wait until the browser delivers a callback, answering anything else with a 404.
    ///
    /// Has no timeout of its own: the caller races it against a deadline and a cancel signal,
    /// because only the caller knows whether the user closed the tab.
    pub async fn accept_callback(&self) -> std::io::Result<Callback> {
        loop {
            let (mut stream, _) = self.listener.accept().await?;

            let mut buf = vec![0u8; MAX_REQUEST_BYTES];
            let mut len = 0;
            // Read until the end of the header block. The request line is all we use, but
            // answering before the browser has finished sending can reset the connection.
            while len < buf.len() {
                let n = stream.read(&mut buf[len..]).await?;
                if n == 0 {
                    break;
                }
                len += n;
                if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }

            let request = String::from_utf8_lossy(&buf[..len]);
            let first_line = request.lines().next().unwrap_or_default();
            let callback = parse_request_line(first_line);

            let (status, body) = match &callback {
                Callback::Code { .. } => ("200 OK", page(true)),
                Callback::Error { .. } => ("200 OK", page(false)),
                Callback::Other => ("404 Not Found", String::new()),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\n\
                 Content-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\n\
                 Cache-Control: no-store\r\n\
                 Referrer-Policy: no-referrer\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            // A browser that gave up before reading the page does not change the outcome.
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;

            if callback != Callback::Other {
                return Ok(callback);
            }
        }
    }
}

/// Parse `GET /callback?code=...&state=... HTTP/1.1`.
pub fn parse_request_line(line: &str) -> Callback {
    let mut parts = line.split_whitespace();
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
        return Callback::Other;
    };

    // The target is origin-form; give it a base so the URL parser handles decoding for us.
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Callback::Other;
    };
    if url.path() != "/callback" {
        return Callback::Other;
    }

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }

    match (error, code, state) {
        (Some(error), _, state) => Callback::Error { error, state },
        (None, Some(code), Some(state)) if !code.is_empty() => Callback::Code { code, state },
        // `/callback` with neither a code nor an error is not a valid response from the server,
        // but it is a request we have to answer rather than hang on.
        _ => Callback::Error {
            error: "invalid_callback".to_string(),
            state: None,
        },
    }
}

/// The page the browser tab lands on. No script, no external resource, nothing to leak the
/// code in the address bar anywhere.
fn page(ok: bool) -> String {
    let (title, line) = if ok {
        (
            "Giriş tamamlandı",
            "SkipFrame'e dönebilirsin. Bu sekmeyi kapatabilirsin.",
        )
    } else {
        (
            "Giriş tamamlanmadı",
            "SkipFrame'e dönüp tekrar deneyebilirsin. Bu sekmeyi kapatabilirsin.",
        )
    };
    format!(
        "<!doctype html><html lang=\"tr\"><head><meta charset=\"utf-8\">\
         <meta name=\"referrer\" content=\"no-referrer\"><title>SkipFrame · {title}</title>\
         <style>html{{background:#0b0b0b;color:#c9ccc6;font:15px/1.6 system-ui,sans-serif}}\
         main{{max-width:28rem;margin:18vh auto;padding:0 1.5rem}}\
         h1{{color:#fff;font-size:1.25rem;margin:0 0 .5rem}}b{{color:#ebb60e}}</style></head>\
         <body><main><h1><b>SkipFrame</b> · {title}</h1><p>{line}</p></main></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_code_and_state() {
        assert_eq!(
            parse_request_line("GET /callback?code=abc123&state=xyz HTTP/1.1"),
            Callback::Code {
                code: "abc123".into(),
                state: "xyz".into()
            }
        );
    }

    #[test]
    fn decodes_percent_encoding() {
        assert_eq!(
            parse_request_line("GET /callback?code=a%2Bb%3D&state=s%20t HTTP/1.1"),
            Callback::Code {
                code: "a+b=".into(),
                state: "s t".into()
            }
        );
    }

    #[test]
    fn an_error_wins_over_everything_else() {
        assert_eq!(
            parse_request_line("GET /callback?error=access_denied&state=xyz HTTP/1.1"),
            Callback::Error {
                error: "access_denied".into(),
                state: Some("xyz".into())
            }
        );
    }

    #[test]
    fn other_paths_are_not_the_callback() {
        assert_eq!(
            parse_request_line("GET /favicon.ico HTTP/1.1"),
            Callback::Other
        );
        assert_eq!(
            parse_request_line("GET /callback/?code=a&state=b HTTP/1.1"),
            Callback::Other
        );
        assert_eq!(
            parse_request_line("POST /callback?code=a&state=b HTTP/1.1"),
            Callback::Other
        );
        assert_eq!(parse_request_line(""), Callback::Other);
    }

    #[test]
    fn a_bare_callback_is_an_error_not_a_hang() {
        assert!(matches!(
            parse_request_line("GET /callback HTTP/1.1"),
            Callback::Error { .. }
        ));
    }

    /// The real thing: bind, have a client hit the redirect URI the way a browser would —
    /// a favicon request first — and get the code back.
    #[tokio::test]
    async fn serves_the_browser_and_returns_the_code() {
        let loopback = Loopback::bind().await.unwrap();
        let uri = loopback.redirect_uri().to_string();
        assert!(uri.starts_with("http://127.0.0.1:") && uri.ends_with("/callback"));

        let base = uri.trim_end_matches("/callback").to_string();
        let browser = tokio::spawn(async move {
            let favicon = raw_get(&base, "/favicon.ico").await;
            assert!(favicon.starts_with("HTTP/1.1 404"));
            let page = raw_get(&base, "/callback?code=c0de&state=st4te").await;
            assert!(page.starts_with("HTTP/1.1 200"));
            assert!(page.contains("Giriş tamamlandı"));
        });

        let got = loopback.accept_callback().await.unwrap();
        browser.await.unwrap();
        assert_eq!(
            got,
            Callback::Code {
                code: "c0de".into(),
                state: "st4te".into()
            }
        );
    }

    async fn raw_get(base: &str, path: &str) -> String {
        let addr = base.trim_start_matches("http://");
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }
}
