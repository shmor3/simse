use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use simse_vnet_engine::backend::NetBackend;
use simse_vnet_engine::error::VnetError;
use simse_vnet_engine::protocol::HttpResponseResult;

use super::channel::read_channel_output;
use super::pool::SshPool;

// ── SshNetBackend ──────────────────────────────────────────────────────────

/// Remote network backend that executes operations via SSH.
///
/// HTTP requests are performed by running `curl` over an exec channel.
/// DNS resolution uses `getent hosts` on the remote machine.
/// WS, TCP, and UDP are not yet implemented over SSH.
pub struct SshNetBackend {
    pool: Arc<SshPool>,
}

impl SshNetBackend {
    /// Create a new `SshNetBackend` backed by the given SSH connection pool.
    pub fn new(pool: Arc<SshPool>) -> Self {
        Self { pool }
    }
}

// ── Shell escaping ─────────────────────────────────────────────────────────

/// Shell-escape a string for safe inclusion in a single-quoted shell argument.
///
/// Wraps the value in single quotes and escapes any embedded single quotes
/// using the `'\''` idiom (end quote, escaped quote, start quote).
fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

// ── curl output parsing ────────────────────────────────────────────────────

/// Parse `curl -i` output into (status, headers, body).
///
/// The `-i` flag causes curl to print response headers followed by a blank
/// line and then the body. The status code is extracted from the first
/// HTTP status line. When curl follows redirects, there may be multiple
/// header blocks — we use the last one.
fn parse_curl_output(
    raw: &str,
) -> Result<(u16, HashMap<String, String>, String), VnetError> {
    // Split into header section and body at the first \r\n\r\n boundary.
    // curl -i uses CRLF line endings in the header block.
    // Find the last header/body boundary (in case of redirects producing
    // multiple header blocks, each is terminated by \r\n\r\n).
    let (header_section, body) = if let Some(pos) = raw.rfind("\r\n\r\n") {
        (&raw[..pos], &raw[pos + 4..])
    } else if let Some(pos) = raw.rfind("\n\n") {
        // Fallback for systems that strip \r
        (&raw[..pos], &raw[pos + 2..])
    } else {
        // No blank line separator — treat entire output as body
        return Ok((0, HashMap::new(), raw.to_string()));
    };

    let mut status: u16 = 0;
    let mut headers = HashMap::new();

    for line in header_section.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with("HTTP/") {
            // e.g. "HTTP/1.1 200 OK" or "HTTP/2 404 Not Found"
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                status = parts[1].parse().unwrap_or(0);
            }
        } else if let Some((key, value)) = line.split_once(':') {
            headers.insert(
                key.trim().to_lowercase(),
                value.trim().to_string(),
            );
        }
    }

    Ok((status, headers, body.to_string()))
}

#[async_trait]
impl NetBackend for SshNetBackend {
    // ── HTTP ────────────────────────────────────────────────────────────

    async fn http_request(
        &self,
        url: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: Option<&str>,
        timeout_ms: u64,
        max_response_bytes: u64,
    ) -> Result<HttpResponseResult, VnetError> {
        let start = Instant::now();

        // Build curl command
        let timeout_secs = (timeout_ms + 999) / 1000; // ceil division
        let mut cmd = format!(
            "curl -s -S -i -X {} --max-time {} --max-filesize {}",
            shell_escape(method),
            timeout_secs,
            max_response_bytes,
        );

        // Add headers
        for (key, value) in headers {
            let header_str = format!("{key}: {value}");
            cmd.push_str(&format!(" -H {}", shell_escape(&header_str)));
        }

        // Add body
        if let Some(body_str) = body {
            cmd.push_str(&format!(" -d {}", shell_escape(body_str)));
        }

        // Add URL (last)
        cmd.push_str(&format!(" {}", shell_escape(url)));

        // Execute over SSH
        let mut channel = self.pool.get_exec_channel().await.map_err(|e| {
            VnetError::ConnectionFailed(format!("SSH exec channel: {e}"))
        })?;

        channel.exec(true, cmd.as_bytes()).await.map_err(|e| {
            VnetError::ConnectionFailed(format!("SSH exec: {e}"))
        })?;

        let output =
            read_channel_output(&mut channel, timeout_ms + 5000, max_response_bytes as usize)
                .await
                .map_err(|e| {
                    if e.to_string().contains("timed out") {
                        VnetError::Timeout(format!(
                            "HTTP request timed out after {timeout_ms}ms"
                        ))
                    } else {
                        VnetError::ConnectionFailed(format!("SSH read: {e}"))
                    }
                })?;

        // Check for curl errors (non-zero exit)
        if output.exit_code.is_some_and(|c| c != 0) {
            let stderr = output.stderr.trim();
            let msg = if stderr.is_empty() {
                format!("curl exited with code {}", output.exit_code.unwrap_or(1))
            } else {
                stderr.to_string()
            };

            // curl exit code 28 = timeout
            if output.exit_code == Some(28) {
                return Err(VnetError::Timeout(format!(
                    "HTTP request timed out after {timeout_ms}ms"
                )));
            }
            // curl exit code 63 = max filesize exceeded
            if output.exit_code == Some(63) {
                return Err(VnetError::ResponseTooLarge(format!(
                    "response exceeds limit {max_response_bytes}"
                )));
            }

            return Err(VnetError::ConnectionFailed(format!(
                "curl failed: {msg}"
            )));
        }

        // Parse response
        let (status, resp_headers, body_str) = parse_curl_output(&output.stdout)?;

        let content_type = resp_headers
            .get("content-type")
            .cloned()
            .unwrap_or_default();

        let body_type = if content_type.contains("json") {
            "json".to_string()
        } else {
            "text".to_string()
        };

        let bytes_received = body_str.len() as u64;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(HttpResponseResult {
            status,
            headers: resp_headers,
            body: body_str,
            body_type,
            duration_ms,
            bytes_received,
        })
    }

    // ── WebSocket (stub) ────────────────────────────────────────────────

    async fn ws_connect(
        &self,
        _url: &str,
        _headers: &HashMap<String, String>,
    ) -> Result<String, VnetError> {
        Err(VnetError::ConnectionFailed(
            "WebSocket over SSH not yet implemented".to_string(),
        ))
    }

    async fn ws_send(&self, _session_id: &str, _data: &str) -> Result<(), VnetError> {
        Err(VnetError::ConnectionFailed(
            "WebSocket over SSH not yet implemented".to_string(),
        ))
    }

    async fn ws_close(&self, _session_id: &str) -> Result<(), VnetError> {
        Err(VnetError::ConnectionFailed(
            "WebSocket over SSH not yet implemented".to_string(),
        ))
    }

    // ── TCP (stub) ──────────────────────────────────────────────────────

    async fn tcp_connect(&self, _host: &str, _port: u16) -> Result<String, VnetError> {
        Err(VnetError::ConnectionFailed(
            "TCP over SSH not yet implemented".to_string(),
        ))
    }

    async fn tcp_send(&self, _session_id: &str, _data: &str) -> Result<(), VnetError> {
        Err(VnetError::ConnectionFailed(
            "TCP over SSH not yet implemented".to_string(),
        ))
    }

    async fn tcp_close(&self, _session_id: &str) -> Result<(), VnetError> {
        Err(VnetError::ConnectionFailed(
            "TCP over SSH not yet implemented".to_string(),
        ))
    }

    // ── UDP (stub) ──────────────────────────────────────────────────────

    async fn udp_send(
        &self,
        _host: &str,
        _port: u16,
        _data: &str,
        _timeout_ms: u64,
    ) -> Result<Option<String>, VnetError> {
        Err(VnetError::ConnectionFailed(
            "UDP over SSH not yet implemented".to_string(),
        ))
    }

    // ── DNS ─────────────────────────────────────────────────────────────

    async fn resolve(&self, hostname: &str) -> Result<Vec<String>, VnetError> {
        let cmd = format!("getent hosts {}", shell_escape(hostname));

        let mut channel = self.pool.get_exec_channel().await.map_err(|e| {
            VnetError::ConnectionFailed(format!("SSH exec channel: {e}"))
        })?;

        channel.exec(true, cmd.as_bytes()).await.map_err(|e| {
            VnetError::ConnectionFailed(format!("SSH exec: {e}"))
        })?;

        let output = read_channel_output(&mut channel, 10_000, 64 * 1024)
            .await
            .map_err(|e| {
                VnetError::DnsResolutionFailed(format!("{hostname}: {e}"))
            })?;

        // Non-zero exit means resolution failed
        if output.exit_code.is_some_and(|c| c != 0) {
            return Err(VnetError::DnsResolutionFailed(format!(
                "no addresses found for {hostname}"
            )));
        }

        // Parse getent output: each line is "IP_ADDRESS hostname [aliases...]"
        let mut ips = Vec::new();
        for line in output.stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // First whitespace-delimited token is the IP address
            if let Some(ip) = line.split_whitespace().next() {
                ips.push(ip.to_string());
            }
        }

        if ips.is_empty() {
            return Err(VnetError::DnsResolutionFailed(format!(
                "no addresses found for {hostname}"
            )));
        }

        Ok(ips)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── shell_escape tests ──────────────────────────────────────────────

    #[test]
    fn shell_escape_simple_string() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn shell_escape_with_spaces() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn shell_escape_with_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn shell_escape_with_special_chars() {
        assert_eq!(
            shell_escape("$(rm -rf /)"),
            "'$(rm -rf /)'",
        );
    }

    #[test]
    fn shell_escape_semicolon_injection() {
        assert_eq!(
            shell_escape("foo; rm -rf /"),
            "'foo; rm -rf /'",
        );
    }

    #[test]
    fn shell_escape_empty_string() {
        assert_eq!(shell_escape(""), "''");
    }

    // ── parse_curl_output tests ─────────────────────────────────────────

    #[test]
    fn parse_curl_output_basic_200() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let (status, headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn parse_curl_output_404() {
        let raw = "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\nNot Found";
        let (status, headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 404);
        assert_eq!(headers.get("content-type").unwrap(), "text/plain");
        assert_eq!(body, "Not Found");
    }

    #[test]
    fn parse_curl_output_http2() {
        let raw = "HTTP/2 301 Moved\r\nLocation: https://example.com\r\n\r\n";
        let (status, headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 301);
        assert_eq!(headers.get("location").unwrap(), "https://example.com");
        assert_eq!(body, "");
    }

    #[test]
    fn parse_curl_output_no_separator() {
        let raw = "just some text without headers";
        let (status, _headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 0);
        assert_eq!(body, "just some text without headers");
    }

    #[test]
    fn parse_curl_output_lf_only() {
        // Fallback for systems that strip \r
        let raw = "HTTP/1.1 200 OK\nContent-Type: text/html\n\nhello";
        let (status, headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(headers.get("content-type").unwrap(), "text/html");
        assert_eq!(body, "hello");
    }

    #[test]
    fn parse_curl_output_multiline_body() {
        let raw = "HTTP/1.1 200 OK\r\n\r\nline1\nline2\nline3";
        let (status, _headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, "line1\nline2\nline3");
    }

    #[test]
    fn parse_curl_output_multiple_headers_same_name() {
        // Last value wins (HashMap behavior)
        let raw = "HTTP/1.1 200 OK\r\nX-Custom: first\r\nX-Custom: second\r\n\r\nbody";
        let (status, headers, body) = parse_curl_output(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(headers.get("x-custom").unwrap(), "second");
        assert_eq!(body, "body");
    }
}
