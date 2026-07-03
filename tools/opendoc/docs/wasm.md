# WebAssembly (WASM) Compilation Guide

This document describes how to compile `opendoc-mcp` as a library for WebAssembly (`wasm32-unknown-unknown`) target.

---

## Prerequisites

Ensure you have the `wasm32-unknown-unknown` target installed for your active Rust toolchain:

```bash
rustup target add wasm32-unknown-unknown
```

---

## Build Command

To compile the library for WebAssembly, you must disable the default features (since the CLI and the tokio-based MCP server require OS APIs and threads not present on standard WASM targets):

```bash
cargo build --target wasm32-unknown-unknown --no-default-features --offline
```

---

## Configuration Details

### 1. Pure-Rust Deflate Compression
The standard `zip` dependency's default features require C-based libraries like `lzma-sys` and `xz2` which fail to compile under WASM due to the lack of a standard C library. In `Cargo.toml`, default features are disabled for `zip` and only the pure-Rust `deflate` compressor is enabled:

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

### 2. Randomness (getrandom)
WebAssembly target does not have a default system source of randomness. Under browser or Node.js runtimes, `getrandom` must be configured to use the JavaScript/browser APIs. In `Cargo.toml`, we configure getrandom's `js`/`wasm_js` backend for WebAssembly targets:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom_v02 = { package = "getrandom", version = "0.2", features = ["js"] }
getrandom_v03 = { package = "getrandom", version = "0.3", features = ["wasm_js"] }
getrandom_v04 = { package = "getrandom", version = "0.4", features = ["wasm_js"] }
```
