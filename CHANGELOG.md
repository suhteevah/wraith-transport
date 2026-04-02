# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-02

### Added

- Initial release, extracted from ClaudioOS bare-metal operating system.
- `HttpRequest` builder for GET and POST requests with header support.
- `HttpResponse` parser with Content-Length and chunked transfer encoding support.
- `decode_chunked` function for RFC 7230 chunked transfer decoding.
- `parse_sse_events` function for Server-Sent Events parsing.
- `ParsedUrl` minimal URL parser for http/https schemes.
- `NetworkBackend` trait for pluggable TCP/TLS network stacks.
- `SmoltcpTransport` for executing HTTP/HTTPS requests over a `NetworkBackend`.
- 11 unit tests covering HTTP serialization, parsing, chunked decoding, SSE, and URL parsing.
