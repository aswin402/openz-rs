# WebUI Remediation TODO

- [x] Restore `web/src/services/websocket.ts` and add `updateSubagentSettings`.
- [x] Fix SettingsModal syntax; run `bun run build`.
- [x] Add typed WebSocket event/envelope contracts and command acknowledgement IDs.
- [x] Enforce WebSocket Origin validation and redact token query logging.
- [x] Bind security approvals to the requesting client and chat; add rejection tests.
- [ ] Replace global WebUI stop with per-turn/session cancellation.
- [ ] Queue/retry configuration writes after reconnect; surface acknowledgements/errors.
- [ ] Reconcile attachment limits, add aggregate quota and TTL cleanup, and validate content server-side.
- [ ] Replace provider/security/channel policy literals with backend capabilities.
- [x] Run focused Rust tests with `just test-one <test> openz` and `just check openz`; avoid full workspace Cargo commands.

## Page repair checklist

- [x] Remove synthetic/hardcoded Cognitive Memory graph nodes and fix stale database schemas, truncation, expired edges, and runtime paths.
- [x] Make Cognitive Memory filters and counts truthful; expose live working-memory keys.
- [x] Add live Skills refresh and runtime skill health counters.
- [x] Add Core Inventory polling and an in-place cron overview with complete job metadata.
- [x] Redesign the Cognitive Memory Graph as a deterministic constellation atlas with Graphify-inspired communities, relation confidence/provenance details, full-record inspection, and bounded canvas rendering for large datasets.
- [x] Verify with `bun run lint` and `bun run build`.
