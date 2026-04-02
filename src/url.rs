//! Minimal URL parser for `no_std` environments.
//!
//! Supports `http://` and `https://` schemes only.

extern crate alloc;

use alloc::format;
use alloc::string::String;

/// Parsed URL components.
pub struct ParsedUrl {
    /// `true` for HTTPS, `false` for HTTP.
    pub is_https: bool,
    /// Hostname (e.g. `"example.com"`).
    pub host: String,
    /// Port number (defaults to 443 for HTTPS, 80 for HTTP).
    pub port: u16,
    /// Path + query string (e.g. `"/api/v1?foo=bar"`).  Defaults to `"/"`.
    pub path: String,
}

/// Parse an HTTP or HTTPS URL into its components.
///
/// Returns an error string if the URL scheme is unsupported or the URL is
/// otherwise malformed.
pub fn parse_url(url: &str) -> Result<ParsedUrl, String> {
    let (is_https, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err(format!("unsupported scheme in: {}", url));
    };

    // Split host from path at the first '/'.
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    // Split host from port at ':'.
    let (host, port) = match host_port.rfind(':') {
        Some(idx) => {
            let port_str = &host_port[idx + 1..];
            let port: u16 = port_str
                .parse()
                .map_err(|_| format!("bad port: {}", port_str))?;
            (&host_port[..idx], port)
        }
        None => {
            let default_port = if is_https { 443 } else { 80 };
            (host_port, default_port)
        }
    };

    if host.is_empty() {
        return Err("empty hostname".into());
    }

    Ok(ParsedUrl {
        is_https,
        host: String::from(host),
        port,
        path: String::from(path),
    })
}
