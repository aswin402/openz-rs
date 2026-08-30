# WebUI Attachment Hardening

## Goal

Make WebSocket attachments safe and consistent by aligning client/server limits, enforcing an aggregate request quota and metadata validation, and removing stale persisted files.

## Design

1. Use shared documented limits: up to 8 files, 8 MiB each, 24 MiB total decoded bytes. Raise the WebSocket frame cap enough for the base64 envelope.
2. Validate MIME type, filename, decoded bytes, and aggregate total on the server; never trust the client-provided size field.
3. Remove attachment files older than 24 hours during persistence, ignoring individual cleanup failures.
4. Enforce the same per-file and aggregate limits in `ChatInput` before reading files.
5. Add focused Rust policy tests and WebUI helper tests; use only scoped checks/builds.

## Completion checklist

- [x] Client and server limits match.
- [x] Aggregate decoded quota is enforced server-side and client-side.
- [x] MIME/name validation rejects unsafe metadata.
- [x] Stale attachment cleanup runs with a bounded TTL.
- [x] Focused verification passes.
