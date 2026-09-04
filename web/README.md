# OpenZ WebUI

React/Vite control center for the OpenZ Rust agent gateway. The WebUI connects over WebSocket, renders chat and agent activity, and exposes settings, skills, memory, inventory, channels, and runtime operations from one console.

## Features

- WebSocket chat with streaming responses, tool execution details, approvals, and orchestration activity.
- Gateway-backed configuration for providers, channels, security, browser ports, skills, and subagents.
- Runtime inventory for sessions, tools, cron jobs, memory databases, paths, and channel status.
- Cognitive memory graph and fact inspection with Canvas rendering.
- Capability and payload normalization at the browser boundary, plus reconnect-safe configuration updates.

---

## 🛠️ Technology Stack

- **Framework**: [React 19](https://react.dev/)
- **Bundler**: [Vite 8](https://vite.dev/)
- **Styling**: [Tailwind CSS v4](https://tailwindcss.com/)
- **State Store**: [Zustand](https://docs.pmnd.rs/zustand)
- **Gateway transport**: native browser WebSocket client
- **Graph rendering**: Canvas-based renderer with derived graph semantics and layout helpers

---

## 🚀 Getting Started

### 1. Installation
```bash
bun install
```

### 2. Run Dev Server
```bash
bun run dev
```

The development server proxies `/ws` to `http://127.0.0.1:8765` by default. Set `VITE_OPENZ_GATEWAY_URL` for another gateway or `VITE_OPENZ_WS_URL` for an explicit WebSocket URL.

### 3. Build Production Bundle
```bash
bun run build
```

---

## Project Structure

```text
src/
├── App.tsx                         # Application composition shell
├── components/                     # Chat, settings, memory, inventory, and feature views
├── config/                         # Runtime and provider registries
├── services/websocket.ts           # Gateway transport and reconnecting event bus
├── shared/                         # Reusable formatters, theme, and UI primitives
├── store/                          # Zustand application and theme state
└── types/                          # Gateway payloads, capabilities, and protocol guards
```

---

## 📜 License
MIT
