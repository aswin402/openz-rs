# ZeroClaw Research & OpenZ Improvement Roadmap 🦀🔍

This document presents a detailed architectural analysis of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw), compares its capabilities with `openz`, and outlines a roadmap of enhancements to elevate `openz` to a production-grade, secure, and highly integrated agent runtime.

---

## 1. ZeroClaw Core Architecture & Features

ZeroClaw is a single-binary Rust agent runtime designed around security, multi-channel flexibility, and hardware integration. Its core features include:

* **OS-Level Sandboxing:** Restricts subprocess/command tool execution using sandboxing frameworks such as Linux Landlock, Bubblewrap, macOS Seatbelt, or Docker containers.
* **Risk Profiles & Approval Gates:** Operates under supervised settings where high-risk or medium-risk actions require user approval (gated by cryptographic **tool receipts**), alongside a YOLO mode for development.
* **SOP (Standard Operating Procedure) Engine:** Implements event-triggered (webhooks, MQTT, cron, or GPIO inputs) workflows that support approval gates, state persistence, and resumable execution.
* **ACP (Agent Client Protocol):** Provides IDE/Editor integration (e.g., VSCode, Zed) using JSON-RPC 2.0 over standard I/O (stdio).
* **Hardware Integration:** Communicates with peripherals (GPIO, I2C, SPI, USB) on boards like Raspberry Pi or ESP32 via a standardized `Peripheral` trait.
* **Web Dashboard:** Bundles an HTTP/WebSocket server rendering a visual UI for real-time chat, configuration editing, memory/fact sheet browsing, and active cron job monitoring.

---

## 2. Gap Analysis: OpenZ vs. ZeroClaw

| Feature Category | ZeroClaw Standard | OpenZ Current State | Gap / Risk |
| :--- | :--- | :--- | :--- |
| **Command Security** | Sandboxed via Bubblewrap/Seatbelt/Docker. | Runs directly on host OS shell via `std::process::Command`. | **High Risk:** LLM code hallucination or tool injections can run destructive commands on the user's host OS. |
| **Approval Gates** | Cryptographic tool receipts and interactive approvals. | Runs commands and writes files without confirmation loops. | **Medium Risk:** Direct writes can overwrite system files or execute untrusted scripts without user review. |
| **IDE/Editor Integration** | Built-in Agent Client Protocol (ACP) over stdio. | Standard terminal CLI and WebSocket long-polling. | No direct integration into VSCode or Zed. |
| **SOP Workflow Engine** | Event-driven (webhooks, MQTT) resumable pipeline. | Basic timer-based cron scheduler. | Cannot automate complex multi-stage event flows. |
| **Hardware Driver** | standard `Peripheral` trait for Raspberry Pi/USB. | None. | Limited to software-only workspace automation. |
| **Visual Dashboard** | Rich visual workspace, memory browser, config editor. | Axum-based WebSocket server without default UI pages. | Requires external WebUI client setup to view states. |

---

## 3. Recommended Improvement Roadmap for OpenZ

To improve OpenZ and bridge the gap with ZeroClaw, we should target the following enhancements:

### Step 1: Implement Shell Sandboxing & Approval Gates (High Priority)
* **Goal:** Protect the host machine from malicious or buggy command execution.
* **Implementation:**
  * Detect the host OS and wrap the `ExecCommandTool` execution.
  * If on Linux, compile commands to run inside `bubblewrap` (bwrap) or docker.
  * Introduce a `supervised` configuration setting. If enabled, intercept write/delete/execute tools and prompt the user in the CLI/WebUI console (`[y/N]`) before proceeding.

### Step 2: Implement Agent Client Protocol (ACP)
* **Goal:** Allow developers to use OpenZ directly inside VSCode, cursor, or Zed.
* **Implementation:**
  * Add an `acp` subcommand (e.g., `openz acp`) to launch a JSON-RPC 2.0 stdio server.
  * Implement standard ACP methods like `initialize`, `chat`, `read_workspace`, and `run_tool`.

### Step 3: Upgrade to a Full SOP Engine
* **Goal:** Enable OpenZ to automate multi-step processes triggered by webhooks or events.
* **Implementation:**
  * Extend the cron module into a generalized Event Engine.
  * Support loading standard operating procedures (JSON/YAML workflows) where states can be paused for user approval and resumed.

### Step 4: Bundle a Default Web Dashboard
* **Goal:** Provide a zero-install visual client out of the box.
* **Implementation:**
  * Serve a pre-compiled static WebUI folder directly from the Axum WebSocket gateway.
  * Expose REST endpoints to read and edit `~/.openz/config.json`, `~/.openz/skills/`, and `~/.openz/sessions/`.
