//! Local HTTP callback server that receives the OAuth redirect from
//! login.live.com and hands the authorization code back to the login flow.
//!
//! Port of `com.mcmanager.client.auth.MicrosoftAuthService.CallbackServer`.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::error::LauncherError;

/// One-shot HTTP server bound to a random localhost port. Listens for the
/// single browser redirect of the OAuth PKCE flow, renders the themed status
/// page and returns the `code` query parameter.
#[derive(Debug)]
pub struct CallbackServer {
    listener: TcpListener,
}

impl CallbackServer {
    /// Binds a listener on `127.0.0.1` with an OS-assigned free port (port 0
    /// → no more 8080 collisions between concurrent launchers).
    pub async fn start() -> Result<Self, LauncherError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        Ok(CallbackServer { listener })
    }

    /// The local port the callback server is listening on.
    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|addr| addr.port()).unwrap_or(0)
    }

    /// Waits up to `timeout` for a single callback request, responds to the
    /// browser with the themed status page, and returns the authorization
    /// code. Returns [`LauncherError::Auth`] on an `error` query parameter, on
    /// a request missing `code`, or when the timeout elapses.
    pub async fn await_code(&mut self, timeout: Duration) -> Result<String, LauncherError> {
        match tokio::time::timeout(timeout, self.accept_once()).await {
            Err(_) => Err(LauncherError::Auth(
                "Login timed out waiting for the browser redirect".to_string(),
            )),
            Ok(result) => result,
        }
    }

    async fn accept_once(&mut self) -> Result<String, LauncherError> {
        let (mut stream, _peer) = self.listener.accept().await?;
        let head = read_request_head(&mut stream).await?;
        let query = parse_callback(&head);
        tracing::debug!(
            "OAuth callback received (code: {}, error: {:?})",
            query.code.is_some(),
            query.error
        );

        let page = callback_page(
            query.code.is_some(),
            query.error.as_deref(),
            query.error_description.as_deref(),
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            page.len(),
            page
        );
        // The response is best-effort; the login outcome depends only on the
        // query parameters (the browser may have already closed the socket).
        let _ = stream.write_all(response.as_bytes()).await;

        match query.code {
            Some(code) => Ok(code),
            None => Err(if let Some(error) = query.error {
                LauncherError::Auth(format!(
                    "Microsoft login failed: {error}{}",
                    query
                        .error_description
                        .map_or(String::new(), |d| format!(" — {d}"))
                ))
            } else {
                LauncherError::Auth("OAuth callback missing code".to_string())
            }),
        }
    }
}

/// Reads the request head (up to the `\r\n\r\n` separator, capped at 16 KiB).
async fn read_request_head(stream: &mut TcpStream) -> Result<String, LauncherError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 16 * 1024 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Query parameters extracted from the callback request.
struct CallbackQuery {
    code: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Parses `code`/`error`/`error_description` out of a callback HTTP request
/// head (request line `GET /callback?code=...&error=... HTTP/1.1`), decoding
/// percent-encoded values (`+` decodes to a space, like Java's `URLDecoder`).
fn parse_callback(head: &str) -> CallbackQuery {
    let query = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|target| target.split_once('?').map(|(_, q)| q))
        .unwrap_or("");
    let mut out = CallbackQuery {
        code: None,
        error: None,
        error_description: None,
    };
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => out.code = Some(value.into_owned()),
            "error" => out.error = Some(value.into_owned()),
            "error_description" => out.error_description = Some(value.into_owned()),
            _ => {}
        }
    }
    out
}

/// Renders the local OAuth callback page in Zircon's dark theme (matching the
/// launcher UI: `#0d1117` background, `#161b22` cards, teal `#47d2c9`
/// accents). Confirms a successful sign-in or surfaces the Azure error
/// returned in the redirect query string.
fn callback_page(success: bool, error: Option<&str>, error_description: Option<&str>) -> String {
    let title = if success {
        "Authentication Successful!"
    } else {
        "Authentication Failed"
    };
    let message = if success {
        "You may now close this browser window and return to the launcher."
    } else {
        "Something went wrong — close this window and return to the launcher."
    };
    let error_html = match error {
        Some(error) => format!(
            "<p class='error'>{}{}</p>",
            escape_html(error),
            error_description.map_or(String::new(), |d| format!(" — {}", escape_html(d)))
        ),
        None => String::new(),
    };
    CALLBACK_PAGE_TEMPLATE
        .replace("__TITLE__", title)
        .replace("__MESSAGE__", message)
        .replace("__ERROR_HTML__", &error_html)
}

/// Minimal HTML escaping so Azure error text can't break out of the page markup.
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CALLBACK_PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { background-color: #0d1117; color: #c9d1d9; font-family: 'Segoe UI', sans-serif; text-align: center; padding-top: 100px; margin: 0; }
        .card { background: #161b22; border: 1px solid #30363d; border-radius: 12px; display: inline-block; padding: 40px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
        .logo { background: #47d2c9; color: #022c29; border-radius: 8px; font-weight: bold; padding: 6px 12px; font-size: 20px; display: inline-block; margin-bottom: 16px; }
        h2 { margin: 0 0 12px 0; color: #ffffff; }
        p { color: #8b949e; font-size: 14px; margin: 0; }
        .error { color: #f85149; margin-top: 12px; }
    </style>
</head>
<body>
    <div class="card">
        <div class="logo">⚡ Zircon</div>
        <h2>__TITLE__</h2>
        <p>__MESSAGE__</p>__ERROR_HTML__
    </div>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_code_from_request_head() {
        let head = "GET /callback?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let query = parse_callback(head);
        assert_eq!(Some("abc123".to_string()), query.code);
        assert_eq!(None, query.error);
        assert_eq!(None, query.error_description);
    }

    #[test]
    fn parses_error_and_decodes_plus_as_space() {
        let head = "GET /callback?error=access_denied&error_description=User+cancelled+the+request HTTP/1.1";
        let query = parse_callback(head);
        assert_eq!(None, query.code);
        assert_eq!(Some("access_denied".to_string()), query.error);
        assert_eq!(
            Some("User cancelled the request".to_string()),
            query.error_description
        );
    }

    #[test]
    fn decodes_percent_encoded_code() {
        let head = "GET /callback?code=a%2Bb%2Fc HTTP/1.1";
        let query = parse_callback(head);
        assert_eq!(Some("a+b/c".to_string()), query.code);
    }

    #[test]
    fn returns_empty_query_for_unrelated_request() {
        let head = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let query = parse_callback(head);
        assert_eq!(None, query.code);
        assert_eq!(None, query.error);
        assert_eq!(None, query.error_description);
    }

    #[test]
    fn escape_html_escapes_markup() {
        assert_eq!(
            "&lt;script&gt;&amp;&quot;x&quot;&lt;/script&gt;",
            escape_html("<script>&\"x\"</script>")
        );
    }
}
