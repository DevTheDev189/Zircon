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
    /// Read deadline for each inbound connection's request head.
    read_timeout: Duration,
}

/// Maximum time a single inbound connection may take to deliver its request
/// head before it is dropped. Stops hung local sockets (Slowloris-style port
/// probes, a wedged browser tab) from blocking the genuine redirect.
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);

impl CallbackServer {
    /// Binds a listener on `127.0.0.1` with an OS-assigned free port (port 0
    /// → no more 8080 collisions between concurrent launchers).
    pub async fn start() -> Result<Self, LauncherError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        Ok(CallbackServer {
            listener,
            read_timeout: REQUEST_READ_TIMEOUT,
        })
    }

    /// The local port the callback server is listening on.
    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or(0)
    }

    /// Waits up to `timeout` for the browser redirect that matches the OAuth
    /// `state` this flow started with, responds to the browser with the themed
    /// status page, and returns the authorization code.
    ///
    /// Requests without the matching one-time `state` (a stale callback, an
    /// unrelated browser tab, or a malicious local process probing the port)
    /// are answered with a failure page and ignored — the server keeps
    /// listening for the genuine redirect until the timeout elapses.
    ///
    /// Returns [`LauncherError::Auth`] on an `error` query parameter, on a
    /// state-matching request missing `code`, or when the timeout elapses.
    pub async fn await_code(
        &mut self,
        timeout: Duration,
        expected_state: &str,
    ) -> Result<String, LauncherError> {
        match tokio::time::timeout(timeout, self.accept_valid(expected_state)).await {
            Err(_) => Err(LauncherError::Auth(
                "Login timed out waiting for the browser redirect".to_string(),
            )),
            Ok(result) => result,
        }
    }

    /// Accepts connections until one carries the expected `state`, then returns
    /// its outcome. Non-matching requests never consume the callback slot.
    async fn accept_valid(&mut self, expected_state: &str) -> Result<String, LauncherError> {
        loop {
            let (mut stream, _peer) = self.listener.accept().await?;

            // Wrap read_request_head in a timeout to prevent hung local sockets
            // from stalling the login redirect: a connection that never sends
            // its request head is dropped and the loop keeps listening.
            let head = match tokio::time::timeout(self.read_timeout, read_request_head(&mut stream))
                .await
            {
                Ok(Ok(head)) => head,
                _ => continue,
            };

            let query = parse_callback(&head);
            tracing::debug!(
                "OAuth callback received (code: {}, state_match: {}, error: {:?})",
                query.code.is_some(),
                query.state.as_deref() == Some(expected_state),
                query.error
            );

            if query.state.as_deref() != Some(expected_state) {
                // Not our flow's redirect — respond and keep listening.
                respond(
                    &mut stream,
                    false,
                    Some("unexpected_request"),
                    Some("This browser tab was not part of a Zircon login."),
                )
                .await;
                continue;
            }

            let outcome = match query.code {
                Some(code) => Ok(code),
                None => Err(if let Some(error) = query.error.clone() {
                    LauncherError::Auth(format!(
                        "Microsoft login failed: {error}{}",
                        query
                            .error_description
                            .clone()
                            .map_or(String::new(), |d| format!(" — {d}"))
                    ))
                } else {
                    LauncherError::Auth("OAuth callback missing code".to_string())
                }),
            };
            respond(
                &mut stream,
                outcome.is_ok(),
                query.error.as_deref(),
                query.error_description.as_deref(),
            )
            .await;
            return outcome;
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
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Parses `code`/`state`/`error`/`error_description` out of a callback HTTP
/// request head (request line `GET /callback?code=...&state=... HTTP/1.1`),
/// decoding percent-encoded values (`+` decodes to a space, like Java's
/// `URLDecoder`).
fn parse_callback(head: &str) -> CallbackQuery {
    let query = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|target| target.split_once('?').map(|(_, q)| q))
        .unwrap_or("");
    let mut out = CallbackQuery {
        code: None,
        state: None,
        error: None,
        error_description: None,
    };
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => out.code = Some(value.into_owned()),
            "state" => out.state = Some(value.into_owned()),
            "error" => out.error = Some(value.into_owned()),
            "error_description" => out.error_description = Some(value.into_owned()),
            _ => {}
        }
    }
    out
}

/// Writes the themed status page to the callback connection. Best-effort: the
/// login outcome depends only on the query parameters, and the browser may
/// have already closed the socket.
async fn respond(
    stream: &mut TcpStream,
    success: bool,
    error: Option<&str>,
    error_description: Option<&str>,
) {
    let page = callback_page(success, error, error_description);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
         Cache-Control: no-store\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        page.len(),
        page
    );
    let _ = stream.write_all(response.as_bytes()).await;
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
    // Only a successful login gets the Microsoft logo row.
    let ms_row = if success {
        "<div class='ms'><svg viewBox='0 0 23 23' xmlns='http://www.w3.org/2000/svg' aria-hidden='true'><path fill='#f35325' d='M0 0h11v11H0z'/><path fill='#81bc06' d='M12 0h11v11H12z'/><path fill='#05a6f0' d='M0 12h11v11H0z'/><path fill='#ffba08' d='M12 12h11v11H12z'/></svg><span>Authenticated with your Microsoft account</span></div>"
            .to_string()
    } else {
        String::new()
    };
    CALLBACK_PAGE_TEMPLATE
        .replace("__TITLE__", title)
        .replace("__MESSAGE__", message)
        .replace("__ERROR_HTML__", &error_html)
        .replace("__MS_ROW__", &ms_row)
}

/// Minimal HTML escaping so Azure error text can't break out of the page markup.
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CALLBACK_PAGE_TEMPLATE: &str = r##"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { background-color: #0d1117; color: #c9d1d9; font-family: 'Segoe UI', sans-serif; text-align: center; padding-top: 100px; margin: 0; }
        .card { background: #161b22; border: 1px solid #30363d; border-radius: 12px; display: inline-block; padding: 40px 48px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
        .title { height: 34px; width: auto; margin: 0 auto 20px; display: block; filter: drop-shadow(0 0 12px rgba(71, 210, 201, 0.35)); }
        h2 { margin: 0 0 12px 0; color: #ffffff; }
        p { color: #8b949e; font-size: 14px; margin: 0; }
        .error { color: #f85149; margin-top: 12px; }
        .ms { display: inline-flex; align-items: center; gap: 8px; margin-top: 18px; padding-top: 16px; border-top: 1px solid #21262d; color: #8b949e; font-size: 12px; }
        .ms svg { width: 16px; height: 16px; }
    </style>
</head>
<body>
    <div class="card">
        <svg class="title" viewBox="0 0 194.96204 60.945377" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Zircon">
            <g transform="translate(-7.8010314,-119.2247)" fill="#47d2c9">
                <path d="m 17.401032,128.8247 v 11.06289 h 4.466394 0.149345 l 3.522265,-4.46794 h 17.74052 v 5.34179 l -25.864055,16.18351 v 3.15898 8.46615 h 42.511617 v -7.88428 -0.58187 -4.47053 h -4.466394 -0.152445 l -3.52485,4.47053 H 34.067198 v -3.15898 l 25.864054,-16.18351 v -5.34179 -6.59495 H 21.867426 20.128514 Z"/>
                <g transform="translate(10.888864,22.929135)">
                    <g transform="translate(-2.2075873,11.305605)">
                        <path d="m 57.695701,100.83351 v 1.45779 3.84886 0.23461 l 2.595708,1.80713 h 7.028511 v -2.04174 -4.29379 -1.01286 z"/>
                        <path d="m 60.291409,110.62723 -2.595708,1.80712 v 0.58136 3.50211 6.55205 8.51215 0.12454 l 2.877344,2.62051 h 6.746875 v -2.74505 -14.61926 -2.90939 -1.03766 -2.38848 z"/>
                    </g>
                    <path d="m 71.545671,118.31445 v 27.31823 h 6.933765 l 3.289556,-3.04271 v -5.39088 l -0.02648,0.0243 v -12.06283 h 7.813159 v 2.87734 h 4.555465 v -0.78238 -3.36931 -2.27531 l -4.555465,-2.63498 h -7.813159 l -3.26307,2.72593 v -3.38739 z"/>
                    <path d="m 103.54753,118.31447 -2.9962,2.01619 v 0.80236 1.0992 2.65154 12.29059 3.68717 1.7153 l 4.46639,3.05586 h 12.97957 2.94659 v -0.69714 -3.30525 -0.69713 l -2.94659,-2.2638 h -8.51266 v -13.7856 h 7.47397 3.98528 v -2.19081 -2.24858 -0.0833 l -3.98528,-2.04664 z"/>
                    <path d="m 132.11922,118.31445 -4.72547,3.34943 v 1.68289 7.3197 7.28705 3.82987 l 6.56315,3.84929 h 12.71377 l 3.73162,-3.89932 v -0.77354 -5.7946 -5.24521 -7.61892 -0.35434 l -5.75682,-3.6323 z m 2.73777,6.5028 h 9.78848 v 1.47046 12.77683 h -9.78848 z m -0.9329,16.96614 h 0.0328 v 0.0193 z"/>
                </g>
                <g transform="matrix(1.0032596,0,0,0.98960683,-0.58809167,1.8003396)">
                    <path d="m 170.05611,140.87977 -2.3151,3.47938 v 1.05161 2.8112 0.95912 19.38073 h 7.70599 v -20.33985 h 7.0776 0.79375 v 17.15038 1.02526 2.16421 h 4.66328 l 3.10885,-1.88516 v -0.27905 -2.33371 -15.80886 -3.22254 -1.59938 l -6.25078,-2.55334 z"/>
                </g>
            </g>
        </svg>
        <h2>__TITLE__</h2>
        <p>__MESSAGE__</p>__ERROR_HTML____MS_ROW__
    </div>
</body>
</html>"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_code_from_request_head() {
        let head = "GET /callback?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let query = parse_callback(head);
        assert_eq!(Some("abc123".to_string()), query.code);
        assert_eq!(Some("xyz".to_string()), query.state);
        assert_eq!(None, query.error);
        assert_eq!(None, query.error_description);
    }

    #[test]
    fn missing_state_parses_as_none() {
        let head = "GET /callback?code=abc123 HTTP/1.1";
        let query = parse_callback(head);
        assert_eq!(Some("abc123".to_string()), query.code);
        assert_eq!(None, query.state);
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

    #[tokio::test]
    async fn await_code_skips_wrong_state_requests() {
        let mut server = CallbackServer::start().await.unwrap();
        let port = server.port();

        let handle = tokio::spawn(async move {
            server
                .await_code(Duration::from_secs(5), "right-state")
                .await
        });

        // A request without the expected state (stale callback, malicious local
        // probe) is answered but must NOT consume the callback slot.
        let mut wrong = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        wrong
            .write_all(
                b"GET /callback?code=stolen&state=wrong-state HTTP/1.1\r\n\
                  Host: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        let _ = wrong.read_to_end(&mut buf).await;
        drop(wrong);

        // The genuine redirect with the right state is accepted.
        let mut right = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        right
            .write_all(
                b"GET /callback?code=the-code&state=right-state HTTP/1.1\r\n\
                  Host: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        let _ = right.read_to_end(&mut buf).await;
        drop(right);

        let code = handle.await.unwrap().expect("no timeout");
        assert_eq!("the-code", code);
    }

    #[tokio::test]
    async fn await_code_rejects_a_state_matching_error() {
        let mut server = CallbackServer::start().await.unwrap();
        let port = server.port();

        let handle =
            tokio::spawn(async move { server.await_code(Duration::from_secs(5), "s").await });
        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        client
            .write_all(
                b"GET /callback?error=access_denied&state=s HTTP/1.1\r\n\
                  Host: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        let _ = client.read_to_end(&mut buf).await;
        drop(client);

        let result = handle.await.unwrap();
        assert!(result.unwrap_err().to_string().contains("access_denied"));
    }

    #[tokio::test]
    async fn stalled_connection_is_dropped_and_does_not_block_the_redirect() {
        let mut server = CallbackServer::start().await.unwrap();
        // Shrink the read deadline so the test runs in milliseconds instead of
        // the production 5 seconds.
        server.read_timeout = Duration::from_millis(200);
        let port = server.port();

        let handle =
            tokio::spawn(async move { server.await_code(Duration::from_secs(5), "s").await });

        // A local port probe connects and never sends a byte. Before the
        // timeout fix this stalled `accept_valid` until the outer await_code
        // deadline — a single hung socket could block a legitimate login.
        let stalled = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        // Wait for the server to hit the read deadline and return to accept().
        tokio::time::sleep(Duration::from_millis(400)).await;
        drop(stalled);

        // The genuine redirect is still accepted afterwards.
        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        client
            .write_all(
                b"GET /callback?code=real-code&state=s HTTP/1.1\r\n\
                  Host: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        let _ = client.read_to_end(&mut buf).await;
        drop(client);

        let code = handle.await.unwrap().expect("no timeout");
        assert_eq!("real-code", code);
    }
}
