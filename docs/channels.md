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
