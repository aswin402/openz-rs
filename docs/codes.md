# OpenZ Coding Guidelines & Standards 🦀⚡

This document outlines the codebase standards, compiler constraints, and async patterns inside `openz`.

---

## 1. Async Design Standards

* **Tokio Runtime:** Async tasks are spawned using `tokio::spawn` and run concurrently in the work-stealing thread pool.
* **Non-Blocking Channel Operations:** WebSocket connections and progress loops communicate via `tokio::sync::mpsc` channels to avoid lifetime/borrow constraints during concurrent writes.
* **Native Thread Safety:** Trait bounds are locked to `Send + Sync` so objects can safely transition across tokio tasks.

---

## 2. Dependency Rules

1. **Pure-Rust SSL:** Always prefer the `rustls-tls` feature inside `reqwest` to bypass external binary dependencies like OpenSSL (`libssl-dev`), making compilation portable.
2. **Object Safety (`async-trait`):** Because native async methods in traits are not currently object-safe in Rust (i.e. they do not build vtables cleanly), traits like `Tool` and `LLMProvider` are decorated with `#[async_trait::async_trait]`.
3. **Serialization:** All schemas (`config/schema.rs`, `session.rs`) derive `Serialize` and `Deserialize` to ensure type-safe JSON conversion.
