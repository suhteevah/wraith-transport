//! HTTP/HTTPS transport over a pluggable network backend.
//!
//! The [`SmoltcpTransport`] struct performs HTTP and HTTPS requests using a
//! user-provided [`NetworkBackend`] implementation.  This decouples the
//! transport logic from any specific network stack, making it usable in
//! any `no_std` environment with a TCP/TLS-capable network stack.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::http::{
    decode_chunked, find_subsequence, header_contains_value, parse_content_length_from_raw,
    HttpRequest, HttpResponse,
};
use crate::url::parse_url;

// ---------------------------------------------------------------------------
// Network backend trait
// ---------------------------------------------------------------------------

/// Errors that a [`NetworkBackend`] implementation can return.
#[derive(Debug)]
pub enum BackendError {
    /// DNS resolution failed.
    DnsError(String),
    /// TCP connection failed.
    TcpError(String),
    /// TLS error (handshake, send, recv).
    TlsError(String),
    /// Read timed out.
    Timeout,
    /// Connection closed by peer (EOF).
    Eof,
    /// The network stack is not ready (e.g. DHCP not complete).
    NotReady,
    /// Other error.
    Other(String),
}

impl core::fmt::Display for BackendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DnsError(msg) => write!(f, "DNS error: {}", msg),
            Self::TcpError(msg) => write!(f, "TCP error: {}", msg),
            Self::TlsError(msg) => write!(f, "TLS error: {}", msg),
            Self::Timeout => write!(f, "timeout"),
            Self::Eof => write!(f, "connection closed (EOF)"),
            Self::NotReady => write!(f, "network not ready"),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

/// Opaque connection handle returned by [`NetworkBackend`].
///
/// The backend assigns meaning to the handle value; the transport treats
/// it as an opaque identifier.
#[derive(Debug, Clone, Copy)]
pub struct ConnectionHandle(pub usize);

/// Trait abstracting the network operations needed by [`SmoltcpTransport`].
///
/// Implement this trait for your smoltcp-based (or other) network stack.
/// The transport calls these methods to perform DNS resolution, establish
/// TCP/TLS connections, and send/receive data.
pub trait NetworkBackend {
    /// Returns `true` if the network stack has an IP address and is ready.
    fn is_ready(&self) -> bool;

    /// Resolve a hostname to an IPv4 address (as 4 bytes).
    fn dns_resolve(&mut self, hostname: &str) -> Result<[u8; 4], BackendError>;

    /// Open a plaintext TCP connection to the given IPv4 address and port.
    ///
    /// Returns a connection handle that can be used with `tcp_send`,
    /// `tcp_recv`, and `tcp_close`.
    fn tcp_connect(&mut self, ip: [u8; 4], port: u16) -> Result<ConnectionHandle, BackendError>;

    /// Send data over a plaintext TCP connection.
    fn tcp_send(&mut self, handle: ConnectionHandle, data: &[u8]) -> Result<(), BackendError>;

    /// Receive data from a plaintext TCP connection.
    ///
    /// Returns the number of bytes read into `buf`.  Returns `Ok(0)` on EOF.
    fn tcp_recv(
        &mut self,
        handle: ConnectionHandle,
        buf: &mut [u8],
    ) -> Result<usize, BackendError>;

    /// Close a plaintext TCP connection.
    fn tcp_close(&mut self, handle: ConnectionHandle);

    /// Open a TLS connection to the given IPv4 address, port, and hostname.
    ///
    /// The implementation should perform the TLS handshake before returning.
    /// Returns a connection handle for TLS send/recv/close.
    fn tls_connect(
        &mut self,
        ip: [u8; 4],
        port: u16,
        hostname: &str,
    ) -> Result<ConnectionHandle, BackendError>;

    /// Send data over a TLS connection.
    fn tls_send(&mut self, handle: ConnectionHandle, data: &[u8]) -> Result<(), BackendError>;

    /// Receive data from a TLS connection.
    ///
    /// Returns the number of bytes read into `buf`.  Returns `Ok(0)` on EOF.
    fn tls_recv(
        &mut self,
        handle: ConnectionHandle,
        buf: &mut [u8],
    ) -> Result<usize, BackendError>;

    /// Close a TLS connection.
    fn tls_close(&mut self, handle: ConnectionHandle);
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from the transport layer.
#[derive(Debug)]
pub enum SmoltcpTransportError {
    /// Failed to parse the request URL.
    InvalidUrl(String),
    /// DNS resolution failed.
    DnsError(String),
    /// TCP connection failed.
    TcpError(String),
    /// TLS error (handshake, send, recv).
    TlsError(String),
    /// HTTP response parsing failed.
    HttpError(String),
    /// The network stack does not have an IP address yet (DHCP incomplete).
    NoNetwork,
    /// Request timed out.
    Timeout,
}

impl core::fmt::Display for SmoltcpTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidUrl(msg) => write!(f, "invalid URL: {}", msg),
            Self::DnsError(msg) => write!(f, "DNS error: {}", msg),
            Self::TcpError(msg) => write!(f, "TCP error: {}", msg),
            Self::TlsError(msg) => write!(f, "TLS error: {}", msg),
            Self::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            Self::NoNetwork => write!(f, "network stack not ready (no IP)"),
            Self::Timeout => write!(f, "request timed out"),
        }
    }
}

impl From<BackendError> for SmoltcpTransportError {
    fn from(e: BackendError) -> Self {
        match e {
            BackendError::DnsError(msg) => SmoltcpTransportError::DnsError(msg),
            BackendError::TcpError(msg) => SmoltcpTransportError::TcpError(msg),
            BackendError::TlsError(msg) => SmoltcpTransportError::TlsError(msg),
            BackendError::Timeout => SmoltcpTransportError::Timeout,
            BackendError::Eof => SmoltcpTransportError::HttpError("unexpected EOF".into()),
            BackendError::NotReady => SmoltcpTransportError::NoNetwork,
            BackendError::Other(msg) => SmoltcpTransportError::HttpError(msg),
        }
    }
}

// ---------------------------------------------------------------------------
// SmoltcpTransport
// ---------------------------------------------------------------------------

/// HTTP/HTTPS transport over a pluggable [`NetworkBackend`].
///
/// This struct owns a mutable reference to the network backend and provides
/// synchronous HTTP request execution with automatic HTTP/HTTPS routing,
/// chunked transfer decoding, and response parsing.
///
/// # Example
///
/// ```rust,ignore
/// let mut transport = SmoltcpTransport::new(my_backend);
/// let headers = BTreeMap::new();
/// let resp = transport.execute_sync("GET", "https://example.com/", &headers, None)?;
/// assert_eq!(resp.status, 200);
/// ```
pub struct SmoltcpTransport<B: NetworkBackend> {
    backend: B,
}

impl<B: NetworkBackend> SmoltcpTransport<B> {
    /// Create a new transport with the given network backend.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Get a reference to the underlying backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Get a mutable reference to the underlying backend.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Execute an HTTP or HTTPS request synchronously.
    ///
    /// Parses the URL, resolves DNS, connects via TCP (or TLS for HTTPS),
    /// sends the request, reads the response, and returns a parsed
    /// [`SmoltcpResponse`].
    pub fn execute_sync(
        &mut self,
        method: &str,
        url: &str,
        headers: &BTreeMap<String, String>,
        body: Option<&[u8]>,
    ) -> Result<SmoltcpResponse, SmoltcpTransportError> {
        let parsed =
            parse_url(url).map_err(|e| SmoltcpTransportError::InvalidUrl(e))?;

        if !self.backend.is_ready() {
            return Err(SmoltcpTransportError::NoNetwork);
        }

        // Step 1: DNS resolution.
        let remote_ip = self
            .backend
            .dns_resolve(&parsed.host)
            .map_err(|e| SmoltcpTransportError::DnsError(format!("{}", e)))?;

        log::info!(
            "[wraith-transport] {} {}:{}{} (resolved: {}.{}.{}.{})",
            method,
            parsed.host,
            parsed.port,
            parsed.path,
            remote_ip[0],
            remote_ip[1],
            remote_ip[2],
            remote_ip[3],
        );

        // Step 2: Build the HTTP/1.1 request bytes.
        let http_req = if let Some(body_bytes) = body {
            HttpRequest::post(&parsed.host, &parsed.path, body_bytes.to_vec())
        } else {
            HttpRequest::get(&parsed.host, &parsed.path)
        };

        // Add headers from the request.
        let mut http_req = http_req;
        for (name, value) in headers {
            // Skip Host header -- HttpRequest adds it automatically.
            if name.eq_ignore_ascii_case("host") {
                continue;
            }
            http_req = http_req.header(name.clone(), value.clone());
        }

        // Add Connection: close so the server closes the connection after
        // sending the response, giving us a clean EOF signal.
        if !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("connection"))
        {
            http_req = http_req.header("Connection", "close");
        }

        // Handle edge case: GET with body.
        let request_bytes = if method == "GET" && body.is_some() {
            let mut req = HttpRequest::get(&parsed.host, &parsed.path);
            req.body = body.map(|b| b.to_vec());
            for (name, value) in headers {
                if name.eq_ignore_ascii_case("host") {
                    continue;
                }
                req = req.header(name.clone(), value.clone());
            }
            if !headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("connection"))
            {
                req = req.header("Connection", "close");
            }
            req.to_bytes()
        } else {
            http_req.to_bytes()
        };

        // Step 3: Connect and send/receive based on HTTP vs HTTPS.
        let response_bytes = if parsed.is_https {
            self.execute_https(remote_ip, parsed.port, &parsed.host, &request_bytes)?
        } else {
            self.execute_http(remote_ip, parsed.port, &request_bytes)?
        };

        // Step 4: Parse the HTTP response.
        self.parse_response(&response_bytes, url)
    }

    /// Perform an HTTPS request via TLS.
    fn execute_https(
        &mut self,
        remote_ip: [u8; 4],
        port: u16,
        hostname: &str,
        request_bytes: &[u8],
    ) -> Result<Vec<u8>, SmoltcpTransportError> {
        // TLS connect + handshake.
        let handle = self
            .backend
            .tls_connect(remote_ip, port, hostname)
            .map_err(|e| SmoltcpTransportError::TlsError(format!("{}", e)))?;

        // Send the HTTP request over TLS.
        self.backend
            .tls_send(handle, request_bytes)
            .map_err(|e| SmoltcpTransportError::TlsError(format!("{}", e)))?;

        log::debug!(
            "[wraith-transport] sent {} byte HTTPS request",
            request_bytes.len()
        );

        // Receive the response.
        let response = self.read_response(handle, true)?;

        // Close the TLS connection.
        self.backend.tls_close(handle);

        Ok(response)
    }

    /// Perform an HTTP (plaintext) request via raw TCP.
    fn execute_http(
        &mut self,
        remote_ip: [u8; 4],
        port: u16,
        request_bytes: &[u8],
    ) -> Result<Vec<u8>, SmoltcpTransportError> {
        // TCP connect.
        let handle = self
            .backend
            .tcp_connect(remote_ip, port)
            .map_err(|e| SmoltcpTransportError::TcpError(format!("{}", e)))?;

        // Send the HTTP request.
        self.backend
            .tcp_send(handle, request_bytes)
            .map_err(|e| SmoltcpTransportError::TcpError(format!("{}", e)))?;

        log::debug!(
            "[wraith-transport] sent {} byte HTTP request",
            request_bytes.len()
        );

        // Receive the response.
        let response = self.read_response(handle, false)?;

        // Close the TCP connection.
        self.backend.tcp_close(handle);

        Ok(response)
    }

    /// Read a full HTTP response from a connection.
    ///
    /// If `is_tls` is true, uses `tls_recv`; otherwise uses `tcp_recv`.
    fn read_response(
        &mut self,
        handle: ConnectionHandle,
        is_tls: bool,
    ) -> Result<Vec<u8>, SmoltcpTransportError> {
        let mut response = Vec::new();
        let mut buf = [0u8; 4096];

        loop {
            let result = if is_tls {
                self.backend.tls_recv(handle, &mut buf)
            } else {
                self.backend.tcp_recv(handle, &mut buf)
            };

            match result {
                Ok(0) => {
                    // EOF -- peer closed.
                    break;
                }
                Ok(n) => {
                    response.extend_from_slice(&buf[..n]);

                    if response_complete(&response) {
                        break;
                    }
                }
                Err(BackendError::Timeout) => {
                    if !response.is_empty() {
                        log::debug!(
                            "[wraith-transport] recv timeout after {} bytes",
                            response.len()
                        );
                        break;
                    }
                    return Err(SmoltcpTransportError::Timeout);
                }
                Err(BackendError::Eof) => {
                    break;
                }
                Err(e) => {
                    if !response.is_empty() {
                        log::debug!(
                            "[wraith-transport] recv error after {} bytes, using partial response",
                            response.len()
                        );
                        break;
                    }
                    return Err(SmoltcpTransportError::from(e));
                }
            }
        }

        Ok(response)
    }

    /// Parse raw HTTP response bytes into our response type.
    fn parse_response(
        &self,
        raw: &[u8],
        original_url: &str,
    ) -> Result<SmoltcpResponse, SmoltcpTransportError> {
        let http_resp = HttpResponse::parse(raw).map_err(|e| {
            SmoltcpTransportError::HttpError(format!("parse error: {:?}", e))
        })?;

        // Check for chunked transfer encoding and decode if needed.
        let body = if http_resp.is_chunked() {
            decode_chunked(&http_resp.body).unwrap_or_else(|_| http_resp.body.clone())
        } else {
            http_resp.body
        };

        // Convert headers from Vec<(String, String)> to BTreeMap.
        let mut headers = BTreeMap::new();
        for (name, value) in &http_resp.headers {
            headers.insert(name.clone(), value.clone());
        }

        Ok(SmoltcpResponse {
            status: http_resp.status,
            headers,
            body,
            url: String::from(original_url),
        })
    }
}

// ---------------------------------------------------------------------------
// Response type
// ---------------------------------------------------------------------------

/// HTTP response from the transport layer.
pub struct SmoltcpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: BTreeMap<String, String>,
    /// Decoded response body.
    pub body: Vec<u8>,
    /// The request URL (no redirect following in this implementation).
    pub url: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check whether the raw response bytes contain a complete HTTP response.
fn response_complete(data: &[u8]) -> bool {
    // Find end of headers.
    let header_end = match find_subsequence(data, b"\r\n\r\n") {
        Some(pos) => pos,
        None => return false,
    };

    let headers = &data[..header_end];
    let body_start = header_end + 4;
    let body = &data[body_start..];

    // Check Content-Length.
    if let Some(cl) = parse_content_length_from_raw(headers) {
        return body.len() >= cl;
    }

    // Check chunked transfer encoding.
    if header_contains_value(headers, b"transfer-encoding", b"chunked") {
        return find_subsequence(body, b"0\r\n\r\n").is_some();
    }

    // Unknown framing -- rely on connection close.
    false
}
