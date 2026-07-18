---
name: prd
description: "Product Requirements Document (PRD) — defines user stories, personas, target features, and success criteria for the project."
---

# Product Requirements Document (PRD)

## 1. Executive Summary
- **Overview**: OpenZ is a local-first Rust AI agent runtime that combines a TUI, background communication channels, native tools, memory, subagents, workflow automation, and MCP support in one binary.
- **Target Audience**: Developers, power users, and automation builders who want a fast local agent for coding, research, file/document/media generation, and multi-channel operation.

## 2. Core Features & Requirements
- [x] Terminal TUI agent with streaming, slash commands, raw-mode-safe rendering, and session persistence.
- [x] Multi-channel operation through WebSocket/WebUI, Telegram, Discord, WhatsApp, and Email.
- [x] Native tool registry covering filesystem, shell, code, git, web, SearchXyz, OpenMedia, OpenDoc, memory, subagents, MCP, and self-management.
- [x] Runtime self-inventory through `openz_inventory` so feature/tool/model answers come from the running binary.
- [x] Managed server lifecycle through `manage_servers` for OpenZ-launched dev servers.
- [x] Persistent memory, knowledge sources, research briefs, reusable workflows, and skill curation.
- [x] Safety controls: approval guard, optional seccomp BPF sandbox, resource policy, audit ledger, and path restrictions.

## 3. Out of Scope
- Marketplace-grade plugin store and MCP discovery UI.
- Full desktop/mobile companion app.
- Production observability dashboard with cost, latency, and trace views.
- First-class Windows/macOS release packaging and signing.

## 4. Success Metrics
- `cargo check` and `cargo test --lib` pass before release.
- `openz_inventory` reports current version, active tools, subagents, channels, runtime identity, and server state.
- Documentation matches the current command/tool surface and avoids stale MCP/native-tool claims.
- User-reported runtime bugs are converted into tests, workflow memory, or skills when repeatable.
