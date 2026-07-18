---
name: design
description: "UI & Design System Specifications — outlines color tokens, typography, layouts, animations, and responsive breakpoints."
---

# UI & Design System Specifications

## 1. Aesthetics
- **Theme**: Terminal-first, dark, dense, and readable. Favor high contrast and compact structure over marketing-heavy visuals.
- **Typography**: Monospace for terminal output, code, tool calls, and logs. Sans-serif is acceptable for WebUI/dashboard surfaces.
- **Colors**: Keep semantic colors stable: success green, warning amber, error red, tool/subagent violet, neutral text gray/white.
- **Tone**: Operational UI should show status, risk, and next action clearly. Avoid decorative effects that reduce scan speed.

## 2. Layout & Responsive Breakpoints
- **Terminal/TUI**: Preserve cursor alignment under raw mode. Use `tui_println!` for terminal output so newlines render correctly.
- **WebUI**: Keep panels dense, resizable, and scroll-safe. Primary views should expose sessions, tools, memory, servers, and logs.
- **Mobile/remote channels**: Keep Telegram/Discord/WhatsApp replies compact and action-oriented. Long output should be summarized with references.

## 3. Interaction Rules
- Approval prompts must accept the first real `Enter` after rendering.
- Cancel behavior must cleanly stop active turns without leaving stale keyboard readers.
- Dev server launches should show the registered server id and clear lifecycle state.
- Feature/tool/model identity answers should use `openz_inventory` before exact claims.

## 4. Animations & Transitions
- TUI animations should be lightweight: spinners, status glyphs, and minimal progress lines.
- WebUI animations may use subtle transitions, but must not block tool monitoring or approval flows.
