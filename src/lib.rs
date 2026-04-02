//! # wraith-transport
//!
//! A `no_std` HTTP/HTTPS transport layer for bare-metal Rust environments.
//!
//! This crate provides:
//!
//! - **`HttpRequest` / `HttpResponse`** — Minimal HTTP/1.1 request builder and
//!   response parser, zero-dependency beyond `alloc`.
//! - **`SmoltcpTransport`** — An HTTP transport implementation designed to work
//!   over smoltcp-based network stacks via the [`NetworkBackend`] trait.
//! - **URL parsing, chunked transfer decoding, SSE parsing** — Utilities for
//!   working with HTTP in `no_std` environments.
//!
//! Originally extracted from [ClaudioOS](https://github.com/suhteevah/claudio-os),
//! a bare-metal Rust operating system.
//!
//! # Usage
//!
//! Implement the [`NetworkBackend`] trait for your smoltcp-based network stack,
//! then construct a [`SmoltcpTransport`] and call [`SmoltcpTransport::execute_sync`].
//!
//! ```rust,ignore
//! use wraith_transport::{SmoltcpTransport, NetworkBackend};
//!
//! // Implement NetworkBackend for your stack...
//! let transport = SmoltcpTransport::new(my_backend);
//! let response = transport.execute_sync("GET", "http://example.com/", &headers, None)?;
//! println!("Status: {}", response.status);
//! ```

#![no_std]

extern crate alloc;

mod http;
mod transport;
mod url;

pub use http::{
    decode_chunked, parse_sse_events, HttpError, HttpRequest, HttpResponse, SseEvent,
};
pub use transport::{NetworkBackend, SmoltcpResponse, SmoltcpTransport, SmoltcpTransportError};
pub use url::ParsedUrl;
