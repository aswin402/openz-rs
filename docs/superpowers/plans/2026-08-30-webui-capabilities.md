# WebUI Capability Contract

## Goal

Make SettingsModal and attachment handling follow the gateway's runtime policy. Provider names, security modes, channel metadata, and upload limits must arrive in config_data rather than being independently hardcoded in the browser.

## Design

1. Add a versioned capabilities object to config_data. Providers reuse the backend model catalog and include display/configuration metadata; channels expose the backend-supported channel names and field defaults; security modes come from one backend constant; attachment limits and allowed MIME types reuse the server policy.
2. Keep sensitive values out of capabilities. The existing masked providers/channels config remains the only editable credential payload.
3. Store capabilities in Zustand with safe empty defaults. SettingsModal derives provider keys, security options, and channel cards/defaults from the payload, falling back to currently loaded config keys while the gateway is still loading.
4. Add focused Rust and WebUI contract tests proving the payload has runtime provider/security/channel/attachment policy and the client consumes it without reintroducing literals.
5. Normalize capability payloads at the client boundary so older or malformed gateway events retain safe nested defaults instead of crashing settings or uploads.

## Completion checklist

- [x] Backend emits a stable capabilities contract in config_data.
- [x] Provider and security-mode options are sourced from capabilities.
- [x] Channel/default metadata is sourced from capabilities.
- [x] Attachment limits/MIME policy can be refreshed from capabilities.
- [x] Focused verification passes.
- [x] Legacy and malformed capability payloads normalize safely.
