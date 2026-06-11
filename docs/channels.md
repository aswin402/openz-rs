# OpenZ Channel Architecture & Configuration 🌐🔌

OpenZ utilizes a modular, trait-driven channel design to connect the core AI execution loop (`AgentLoop`) with external platforms and APIs (such as the terminal, WebUI, Telegram, Discord, and WhatsApp).

---

## 1. The `Channel` Trait

All communication channels implement the asynchronous `Channel` trait defined in [src/channels/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs):

```rust
use async_trait::async_trait;

#[async_trait]
pub trait Channel: Send + Sync {
    /// The unique system name for this channel (e.g. "cli", "telegram", "websocket", "discord", "whatsapp")
    fn name(&self) -> &'static str;

    /// Runs/starts the listener loop for the channel
    async fn start(&self) -> anyhow::Result<()>;
}
```

---

## 2. Supported Channels

* **`cli`** ([src/channels/cli.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli.rs)): Interactive TUI terminal prompt support with clipboard image pasting (`Ctrl+V`) and agent slash commands.
* **`websocket`** ([src/channels/websocket.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket.rs)): Axum-based WebSocket gateway that serves WebUI static bundles and accepts real-time message events.
* **`telegram`** ([src/channels/telegram.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram.rs)): Standard long-polling bot polling messenger messages.
* **`discord`** ([src/channels/discord.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord.rs)): Pluggable adapter stub to run a Discord bot channel.
* **`whatsapp`** ([src/channels/whatsapp.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs)): Pluggable webhook adapter stub to connect to WhatsApp Business API.

---

## 3. Interactive Configuration CLI Wizard

Users can configure channels and providers by running:

```bash
openz configure
```

The menu options will guide you through:
1. **Providers:** Select provider, models, bases, bot name, and icon.
2. **Gateway/WebSocket:** Enable gateway, set port/host, and select auto-start preference.
3. **Telegram:** Enable Telegram and set the bot token.
4. **Discord:** Enable Discord and set the bot token.
5. **WhatsApp:** Enable WhatsApp, set API Key, and set Phone Number ID.

---

## 4. Gateway Auto-Start Settings

When configuring the Gateway, OpenZ offers two automated background runner preferences:

### Option 1: System Boot Daemon (systemd service)
Automatically generates, enables, and starts a user-level `systemd` service unit:
* Service location: `~/.config/systemd/user/openz-gateway.service`
* Managed programmatically using standard `systemctl --user` commands:
  ```bash
  systemctl --user daemon-reload
  systemctl --user enable openz-gateway.service
  systemctl --user restart openz-gateway.service
  ```
* Ensure lingering is enabled (`loginctl enable-linger`) if you want the user service to start even before logging into the desktop environment.

### Option 2: Auto-Start on TUI Launch
Launches the WebSocket gateway server on an asynchronous `tokio` background thread immediately when you start the terminal TUI (`openz agent`). This allows you to open a browser workbench without launching separate gateway processes.

---

## 5. Console CLI TUI Enhancements ⚡💻

The console CLI features a robust raw input loop with high-fidelity visual logs and indicators:
* **Clean Pasting Placeholders**: Pasting images (`Ctrl+V` or `Alt+V`) inserts neat inline placeholders like `[image]`, `[image1]`, `[image2]`, etc. These placeholders are resolved under the hood to their respective full markdown file syntax before sending the request to the agent, keeping the user input prompt clean and readable.
* **Narrow Window Protection**: The bottom status bar automatically elides the model name based on your terminal's column width to prevent text wrapping from breaking vertical cursor alignments.
* **Color-Themed Activity Indicators**:
  * `✕ Error`: Red logs (`ERROR_RED`) for error prompts or execution failures.
  * `✓ Success`: Emerald green logs (`EMERALD_GREEN`) for successful operations and completion checkmarks.
  * `▲ Warning`: Yellow logs (`AURA_GOLD`) for unsupported images or alert warnings.
  * `▸ Tool`: Violet logs (`AURA_PURPLE`) for tool executions (planning, writing, command execution).
  * `◎ Subagent`: Violet logs (`AURA_PURPLE`) for specialized and general subagent spawns (e.g. `◎ Vision Agent`).

---

## 6. Configuration Schema Details (`config.json`)

The active configuration file is located at `~/.openz/config.json`. Below are the key configuration fields under `"agents" -> "defaults"`:

* **`workspace`**: Path to local workspace directory for file and command executions.
* **`model`**: Name of the active default LLM model.
* **`provider`**: API provider (e.g. `openai`, `anthropic`, `openrouter`, `deepseek`, `groq`, `ollama`).
* **`maxTokens`**: Maximum output tokens for LLM generation.
* **`temperature`**: Generation temperature (typically `0.1` to `1.0`).
* **`botName`**: Display name of the bot.
* **`botIcon`**: Icon emoji/symbol for the bot.
* **`maxMessages`**: Context window conversation length history limit.
* **`maxToolIterations`**: Maximum nested tool execution iterations per prompt.
* **`cavemanMode`**: Toggle built-in prompt compression.
* **`contextLimit`** *(Optional)*: Explicitly overrides the default hardcoded context limit (in tokens) shown in the terminal status bar (e.g., set to `128000` or `1000000`). If omitted, default limits are dynamically resolved based on the active model name.

---

## 7. Lifecycle Messages & Offline Notifications 🔔💤

To keep users updated on agent availability across external platforms (Telegram, Discord, WhatsApp), OpenZ manages active/offline notifications:

* **Active (Startup) Messages**: When a channel listener starts, it sends a randomized greeting message selected from `ACTIVE_MESSAGES` (without any visual indicator prefix like `🟢 Active` to keep it clean and minimal).
* **Offline (Shutdown) Messages**: When the TUI exits cleanly (via `/exit` command or keyboard interrupts like `Ctrl+C` or `Ctrl+D`), it invokes `shutdown_gateways` to notify all active channels with a random deactivation message from `OFFLINE_MESSAGES`.
* **Clean Shutdown Integration**: The TUI raw input processor catches `Ctrl+C`/`Ctrl+D` interrupts, disables raw terminal mode, and signals a clean exit to the agent runner rather than terminating the process abruptly. This guarantees that background connections are notified of deactivation before the process shuts down.
* **Diagnostics**: If an offline notification fails to deliver, the error details and API response status are logged to stderr to help with troubleshooting channel configurations.

