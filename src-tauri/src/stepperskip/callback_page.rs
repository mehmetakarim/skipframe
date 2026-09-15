//! The page the browser tab shows when StepperSkip sends the user back to SkipFrame.
//!
//! It is written after the sign-in has actually finished — code exchanged, token stored, account
//! read — so it can say what happened rather than what was hoped. An earlier version answered as
//! soon as the code arrived, and would say "complete" in the browser while the app, a moment
//! later, reported that the exchange had failed.
//!
//! Self-contained on purpose: no font, image or stylesheet is fetched from anywhere. The tab is
//! served from `127.0.0.1` with an authorization code in its address, and nothing on it should
//! make a request, to StepperSkip or to anyone.
//!
//! It follows the browser's light or dark preference. Every colour comes from one set of
//! variables that both schemes redefine — StepperSkip's consent screen hard-coded dark-theme text
//! over a theme-dependent card and lost the application's name in light mode, and the same
//! mistake is not repeated here.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use super::api::{Company, User};

/// What to tell the user.
pub enum Outcome<'a> {
    SignedIn {
        user: &'a User,
        companies: &'a [Company],
    },
    Failed {
        code: &'a str,
    },
}

pub fn render(outcome: &Outcome<'_>) -> String {
    let (title, status_class, icon, body) = match outcome {
        Outcome::SignedIn { user, companies } => (
            "Giriş tamamlandı",
            "ok",
            ICON_CHECK,
            signed_in_body(user, companies),
        ),
        Outcome::Failed { code } => ("Giriş tamamlanmadı", "bad", ICON_CROSS, failed_body(code)),
    };

    format!(
        r#"<!doctype html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark light">
<meta name="referrer" content="no-referrer">
<title>SkipFrame · {title}</title>
<link rel="icon" href="data:image/svg+xml;base64,{favicon}">
<style>{STYLE}</style>
</head>
<body>
<main class="card">
  <header class="brand">{MARK}<span class="wordmark">SkipFrame</span><span class="by" lang="en">× STEPPERSKIP</span></header>
  <section class="status {status_class}">
    <span class="badge" aria-hidden="true">{icon}</span>
    <h1>{title}</h1>
  </section>
  {body}
</main>
<script>
  // The authorization code is single-use and useless without the verifier SkipFrame kept, but
  // there is still no reason to leave it in the address bar or in this tab's history entry.
  history.replaceState(null, "", location.pathname);
</script>
</body>
</html>"#,
        favicon = STANDARD.encode(FAVICON_SVG),
    )
}

fn signed_in_body(user: &User, companies: &[Company]) -> String {
    let handle = format!("@{}", escape(&user.username));
    // StepperSkip falls back to the username when a profile has no name; saying it twice reads
    // like a mistake.
    let who = if user.display_name.trim().is_empty() || user.display_name == user.username {
        format!(r#"<span class="mono">{handle}</span>"#)
    } else {
        format!(
            r#"{} <span class="mono muted">{handle}</span>"#,
            escape(&user.display_name)
        )
    };

    let publishable: Vec<&Company> = companies.iter().filter(|c| c.can_publish).collect();
    let share = match publishable.as_slice() {
        [] => r#"<span class="warn">Firma profili yok — paylaşım için gerekli</span>"#.to_string(),
        [only] => escape(&only.name),
        many => format!("{} firma profili · paylaşırken seçeceksin", many.len()),
    };

    format!(
        r#"<p class="lead">StepperSkip hesabın SkipFrame’e bağlandı.</p>
  <dl class="rows">
    <div><dt>Hesap</dt><dd>{who}</dd></div>
    <div><dt>Paylaşım</dt><dd>{share}</dd></div>
  </dl>
  <p class="next">Bu sekmeyi kapatıp <strong>SkipFrame</strong>’e dönebilirsin.</p>
  <p class="fine">SkipFrame parolanı görmedi. Bağlantıyı istediğin zaman SkipFrame’de Ayarlar → Hesap’tan kaldırabilirsin.</p>"#
    )
}

fn failed_body(code: &str) -> String {
    // Only the cases a user can act on from the browser get their own sentence. Everything else
    // is explained, in full and in Turkish, by the notice in the app.
    let reason = match code {
        "access_denied" => "StepperSkip’te izin verilmedi.",
        "state_mismatch" => {
            "Bu yanıt SkipFrame’in başlattığı girişe ait değildi, güvenlik için durduruldu."
        }
        "invalid_grant" => "Giriş kodu geçersiz ya da süresi dolmuştu.",
        "network" | "timeout" => "SkipFrame, StepperSkip’e ulaşamadı.",
        _ => "Ayrıntı SkipFrame’de görünüyor.",
    };
    format!(
        r#"<p class="lead">{reason}</p>
  <p class="next"><strong>SkipFrame</strong>’e dönüp yeniden deneyebilirsin. Bu sekmeyi kapatabilirsin.</p>"#
    )
}

/// Account and company names come from the server; they are text, never markup.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The framed mark from the logo kit (`src/components/SfMark.vue`), full level of detail.
const MARK: &str = r##"<svg class="mark" viewBox="0 0 100 100" width="36" height="36" role="img" aria-label="SkipFrame"><rect x="10" y="10" width="80" height="80" rx="10" fill="none" stroke="currentColor" stroke-width="10"/><g fill="currentColor"><rect x="25" y="72" width="50" height="10"/><rect x="30" y="57" width="40" height="10"/><rect x="35" y="42" width="30" height="10"/></g><rect x="40" y="27" width="20" height="10" fill="#ebb60e"/></svg>"##;

/// The kit's simplified level, which is the one meant for small sizes like a tab icon.
const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" rx="22" fill="#0b0b0b"/><rect x="18" y="18" width="64" height="64" rx="7" fill="none" stroke="#fff" stroke-width="12"/><rect x="33" y="55" width="34" height="15" fill="#fff"/><rect x="40" y="34" width="20" height="15" fill="#ebb60e"/></svg>"##;

const ICON_CHECK: &str = r#"<svg viewBox="0 0 24 24" width="18" height="18"><path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

const ICON_CROSS: &str = r#"<svg viewBox="0 0 24 24" width="18" height="18"><path d="M7 7l10 10M17 7L7 17" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"/></svg>"#;

/// SkipFrame's own tokens (`src/styles/tokens.css`) for dark; a light counterpart built the same
/// way. Gold is used only on the mark: as text on a light ground it does not reach a readable
/// contrast.
const STYLE: &str = r#"
:root{--bg:#0b0b0b;--card:#101010;--line:#2a2a2a;--title:#ffffff;--text:#c9ccc6;--muted:#8a8f88;
--mark:#ffffff;--ok:#4fbb72;--ok-bg:rgba(79,187,114,.12);--bad:#e05a45;--bad-bg:rgba(224,90,69,.12);--warn:#ebb60e}
@media (prefers-color-scheme: light){:root{--bg:#f1f1ee;--card:#ffffff;--line:#e3e4e0;--title:#101010;
--text:#3f4441;--muted:#6b706a;--mark:#101010;--ok:#237a42;--ok-bg:rgba(35,122,66,.10);--bad:#b23a28;
--bad-bg:rgba(178,58,40,.10);--warn:#8a6a00}}
*{box-sizing:border-box}
html,body{margin:0;min-height:100%;background:var(--bg);color:var(--text)}
body{display:flex;align-items:center;justify-content:center;min-height:100vh;padding:24px;
font:15px/1.6 'Space Grotesk',ui-sans-serif,system-ui,-apple-system,'Segoe UI',sans-serif}
.card{width:100%;max-width:440px;background:var(--card);border:1px solid var(--line);border-radius:10px;
padding:28px 28px 24px;box-shadow:0 18px 50px rgba(0,0,0,.18)}
.brand{display:flex;align-items:center;gap:10px;color:var(--mark);margin-bottom:26px}
.wordmark{font-weight:700;font-size:17px;letter-spacing:-.01em;color:var(--title)}
.by{margin-left:auto;font-size:11px;letter-spacing:.12em;color:var(--muted)}
.status{display:flex;align-items:center;gap:12px;margin-bottom:6px}
.badge{display:inline-flex;align-items:center;justify-content:center;width:34px;height:34px;border-radius:50%;flex:none}
.ok .badge{color:var(--ok);background:var(--ok-bg)}
.bad .badge{color:var(--bad);background:var(--bad-bg)}
h1{margin:0;font-size:21px;line-height:1.3;color:var(--title)}
.lead{margin:0 0 18px 46px;color:var(--text)}
.rows{margin:0 0 20px;border-top:1px solid var(--line)}
.rows div{display:flex;gap:16px;padding:11px 0;border-bottom:1px solid var(--line)}
dt{width:76px;flex:none;font-size:11px;letter-spacing:.12em;text-transform:uppercase;color:var(--muted);padding-top:3px}
dd{margin:0;color:var(--title);min-width:0;overflow-wrap:anywhere}
.mono{font-family:'JetBrains Mono',ui-monospace,'Cascadia Code','SF Mono',Menlo,monospace;font-size:13.5px}
.muted{color:var(--muted)}
.warn{color:var(--warn)}
.next{margin:0 0 8px;color:var(--text)}
.next strong{color:var(--title)}
.fine{margin:0;font-size:12.5px;color:var(--muted)}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn user(display: &str) -> User {
        User {
            id: 97,
            username: "mehmetakarim".into(),
            display_name: display.into(),
            avatar_url: None,
            profile_url: None,
        }
    }

    fn company(name: &str) -> Company {
        Company {
            id: 1,
            name: name.into(),
            slug: "teknovada".into(),
            logo_url: None,
            profile_url: None,
            is_active: true,
            can_publish: true,
        }
    }

    #[test]
    fn says_who_signed_in_and_where_shares_will_go() {
        let companies = [company("TeknoVada")];
        let page = render(&Outcome::SignedIn {
            user: &user("Mehmet Akarım"),
            companies: &companies,
        });
        assert!(page.contains("Giriş tamamlandı"));
        assert!(page.contains("Mehmet Akarım"));
        assert!(page.contains("@mehmetakarim"));
        assert!(page.contains("TeknoVada"));
    }

    #[test]
    fn does_not_repeat_a_username_standing_in_for_a_name() {
        let page = render(&Outcome::SignedIn {
            user: &user("mehmetakarim"),
            companies: &[],
        });
        assert_eq!(page.matches("mehmetakarim").count(), 1);
        assert!(page.contains("Firma profili yok"));
    }

    #[test]
    fn server_text_is_escaped_not_rendered() {
        let companies = [company("<img src=x onerror=alert(1)>")];
        let page = render(&Outcome::SignedIn {
            user: &user("\"><script>alert(1)</script>"),
            companies: &companies,
        });
        assert!(!page.contains("<img src=x"));
        assert!(!page.contains("<script>alert(1)"));
        assert!(page.contains("&lt;img src=x onerror=alert(1)&gt;"));
    }

    /// `text-transform: uppercase` on a `lang="tr"` page turns "StepperSkip" into "STEPPERSKİP".
    /// Right for Turkish words, wrong for a brand; the brand is written in capitals instead.
    #[test]
    fn the_brand_is_not_uppercased_by_turkish_rules() {
        let page = render(&Outcome::Failed { code: "x" });
        assert!(page.contains(">× STEPPERSKIP<"));
        let rule = STYLE
            .split(".by{")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(!rule.contains("text-transform"));
    }

    #[test]
    fn a_failure_says_so() {
        let page = render(&Outcome::Failed {
            code: "access_denied",
        });
        assert!(page.contains("Giriş tamamlanmadı"));
        assert!(page.contains("izin verilmedi"));
        assert!(!page.contains("Giriş tamamlandı"));
    }

    /// Nothing on the page may reach the network: no remote stylesheet, font, image or script.
    /// (The SVG namespace inside the data-URI favicon is an identifier, not a request.)
    #[test]
    fn loads_nothing_from_anywhere() {
        let companies = [company("TeknoVada")];
        let page = render(&Outcome::SignedIn {
            user: &user("Mehmet"),
            companies: &companies,
        });
        for fetch in [
            "src=\"http",
            "href=\"http",
            "url(http",
            "@import",
            "<link rel=\"stylesheet",
        ] {
            assert!(!page.contains(fetch), "page would fetch: {fetch}");
        }
        assert!(page.contains("href=\"data:image/svg+xml;base64,"));
    }

    /// Both colour schemes define every variable the rules use — the consent-screen bug, caught
    /// by construction rather than by eye.
    #[test]
    fn both_colour_schemes_define_the_same_variables() {
        let (dark, light) = STYLE
            .split_once("@media (prefers-color-scheme: light)")
            .unwrap();
        let names = |css: &str| -> std::collections::BTreeSet<String> {
            css.split("--")
                .skip(1)
                .filter_map(|s| s.split(':').next())
                .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
                .map(str::to_string)
                .collect()
        };
        let dark_vars = names(dark.split('}').next().unwrap());
        let light_vars = names(light.split("}}").next().unwrap());
        assert!(!dark_vars.is_empty());
        assert_eq!(dark_vars, light_vars);

        // And no rule hard-codes a colour where a variable should be.
        let rules = light.split_once("}}").unwrap().1;
        assert!(!rules.contains("color:#"), "a rule uses a literal colour");
    }

    /// Writes every variant to `SKIPFRAME_PAGE_PREVIEW_DIR` to be looked at in a browser, in both
    /// colour schemes. Not a check — run it when the page's design changes:
    /// `SKIPFRAME_PAGE_PREVIEW_DIR=/some/dir cargo test -p skipframe write_previews -- --ignored`
    #[test]
    #[ignore = "writes preview files; set SKIPFRAME_PAGE_PREVIEW_DIR"]
    fn write_previews() {
        let dir = std::path::PathBuf::from(std::env::var("SKIPFRAME_PAGE_PREVIEW_DIR").unwrap());
        let one = [company("TeknoVada")];
        let two = [company("TeknoVada"), company("Birdata")];
        let pages = [
            (
                "signed-in",
                render(&Outcome::SignedIn {
                    user: &user("Mehmet Akarım"),
                    companies: &one,
                }),
            ),
            (
                "signed-in-username-only",
                render(&Outcome::SignedIn {
                    user: &user("mehmetakarim"),
                    companies: &one,
                }),
            ),
            (
                "signed-in-no-company",
                render(&Outcome::SignedIn {
                    user: &user("mehmetakarim"),
                    companies: &[],
                }),
            ),
            (
                "signed-in-two-companies",
                render(&Outcome::SignedIn {
                    user: &user("Mehmet Akarım"),
                    companies: &two,
                }),
            ),
            (
                "denied",
                render(&Outcome::Failed {
                    code: "access_denied",
                }),
            ),
            (
                "other-failure",
                render(&Outcome::Failed {
                    code: "bad_response",
                }),
            ),
        ];
        for (name, html) in pages {
            std::fs::write(dir.join(format!("{name}.html")), html).unwrap();
        }
    }
}
