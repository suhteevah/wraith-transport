# wraith-transport

[![Crates.io](https://img.shields.io/crates/v/wraith-transport.svg)](https://crates.io/crates/wraith-transport)
[![Docs.rs](https://docs.rs/wraith-transport/badge.svg)](https://docs.rs/wraith-transport)
[![License](https://img.shields.io/crates/l/wraith-transport.svg)](LICENSE-MIT)

A `no_std` HTTP/HTTPS transport layer for bare-metal Rust environments.

Originally extracted from [ClaudioOS](https://github.com/suhteevah/claudio-os), a bare-metal Rust operating system that runs AI coding agents directly on hardware with no Linux kernel, no POSIX layer, and no JavaScript runtime.

## Features

- **`#![no_std]`** with only `alloc` dependency -- works on bare metal, embedded, and WASM targets
- **HTTP/1.1 request builder** -- GET, POST, and arbitrary methods with header support
- **HTTP/1.1 response parser** -- status line, headers, body extraction, Content-Length and chunked transfer encoding
- **Chunked transfer decoding** -- RFC 7230 compliant chunk-by-chunk decoder
- **SSE (Server-Sent Events) parser** -- parse streaming `event:` / `data:` fields
- **URL parser** -- minimal `http://` and `https://` URL parsing
- **Pluggable network backend** -- implement the `NetworkBackend` trait for your TCP/TLS stack (smoltcp, embassy-net, etc.)
- **Zero unsafe in transport logic** -- all unsafe is confined to your backend implementation

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
wraith-transport = "0.1"
```

### Standalone HTTP parsing (no network backend needed)

```rust
use wraith_transport::{HttpRequest, HttpResponse, decode_chunked, parse_sse_events};

// Build a request
let req = HttpRequest::post("api.example.com", "/v1/data", b"{\"key\":\"value\"}".to_vec())
    .header("Content-Type", "application/json")
    .header("Authorization", "Bearer token123");
let bytes = req.to_bytes(); // Raw HTTP/1.1 bytes ready to send

// Parse a response
let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
let resp = HttpResponse::parse(raw).unwrap();
assert_eq!(resp.status, 200);
assert_eq!(resp.body, b"OK");

// Parse SSE events
let sse_data = b"event: message\ndata: hello world\n\n";
let (events, _consumed) = parse_sse_events(sse_data);
assert_eq!(events[0].data, "hello world");
```

### Full transport with a network backend

```rust,ignore
use wraith_transport::{
    NetworkBackend, BackendError, ConnectionHandle,
    SmoltcpTransport, SmoltcpResponse,
};
use alloc::collections::BTreeMap;

struct MySmoltcpBackend { /* your smoltcp stack */ }

impl NetworkBackend for MySmoltcpBackend {
    fn is_ready(&self) -> bool { /* check DHCP lease */ }
    fn dns_resolve(&mut self, hostname: &str) -> Result<[u8; 4], BackendError> { /* ... */ }
    fn tcp_connect(&mut self, ip: [u8; 4], port: u16) -> Result<ConnectionHandle, BackendError> { /* ... */ }
    fn tcp_send(&mut self, handle: ConnectionHandle, data: &[u8]) -> Result<(), BackendError> { /* ... */ }
    fn tcp_recv(&mut self, handle: ConnectionHandle, buf: &mut [u8]) -> Result<usize, BackendError> { /* ... */ }
    fn tcp_close(&mut self, handle: ConnectionHandle) { /* ... */ }
    fn tls_connect(&mut self, ip: [u8; 4], port: u16, hostname: &str) -> Result<ConnectionHandle, BackendError> { /* ... */ }
    fn tls_send(&mut self, handle: ConnectionHandle, data: &[u8]) -> Result<(), BackendError> { /* ... */ }
    fn tls_recv(&mut self, handle: ConnectionHandle, buf: &mut [u8]) -> Result<usize, BackendError> { /* ... */ }
    fn tls_close(&mut self, handle: ConnectionHandle) { /* ... */ }
}

let backend = MySmoltcpBackend { /* ... */ };
let mut transport = SmoltcpTransport::new(backend);

let headers = BTreeMap::new();
let resp = transport.execute_sync("GET", "https://example.com/", &headers, None).unwrap();
assert_eq!(resp.status, 200);
```

## Architecture

```
  Your Application
        |
  SmoltcpTransport
    |         |
  URL       HTTP
  Parser    Request/Response
    |         |
  NetworkBackend (trait)
    |
  Your TCP/TLS Stack
  (smoltcp, embassy-net, etc.)
```

The crate is split into layers:

1. **`http`** module -- pure HTTP/1.1 serialization/parsing with no I/O dependencies
2. **`url`** module -- minimal URL parser for http/https schemes
3. **`transport`** module -- orchestrates DNS, TCP/TLS connect, request send, response receive via the `NetworkBackend` trait

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

## Contributing

Contributions welcome! Please open an issue or PR on [GitHub](https://github.com/suhteevah/wraith-transport).

---

---

---

---

---

---

---

---

---

---

---

---

---

---

---

---

## Support This Project

If you find this project useful, consider buying me a coffee! Your support helps me keep building and sharing open-source tools.

[![Donate via PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg?logo=paypal)](https://www.paypal.me/baal_hosting)

**PayPal:** [baal_hosting@live.com](https://paypal.me/baal_hosting)

Every donation, no matter how small, is greatly appreciated and motivates continued development. Thank you!
