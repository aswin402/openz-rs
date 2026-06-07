# Original Nanobot Chat Channels Reference 🐍

This document preserves details about the original Python channels supported by `nanobot` to aid in future porting to Rust.

---

## 1. Supported Channels

* **Discord (`discord.py`)**: Uses the `discord.py` package. Listens to message events, handles threads, handles typing indicators, and splits long messages.
* **Feishu / Lark (`feishu.py`)**: Massive integration supporting Feishu card templates, rich text formatting, Feishu threads, media attachments, and Feishu voice messages.
* **DingTalk (`dingtalk.py`)**: Uses DingTalk Stream API. Supports rich media messaging and link authorization.
* **Slack (`slack.py`)**: Uses `slack-sdk` and `slackify-markdown`. Renders markdown messages and handles threads.
* **Matrix (`matrix.py`)**: Uses `matrix-nio[e2e]`. Supports end-to-end encrypted chats and media uploads.
* **WeChat / Weixin (`weixin.py`, `wecom.py`, `mochat.py`)**: Supports Personal WeChat, Enterprise WeChat (WeCom), and third-party WeChat bridges. Handles voice, media, and QR code pairing.
* **QQ / NapCat (`qq.py`, `napcat.py`)**: Uses NapCat/go-cqhttp protocol. Listens to NapCat WebSocket triggers and replies using CQCodes.
* **Email (`email.py`)**: Connects to IMAP for reading inbox messages and SMTP for sending responses. Supports file attachments.
* **MS Teams (`msteams.py`)**: Connects to Microsoft Graph API / Bot Framework for channels/chats.
* **Signal (`signal.py`)**: Connects to `signal-cli` json-rpc socket to send/receive messages.
* **WhatsApp (`whatsapp.py`)**: Connects to WhatsApp Business API / Baileys bridge.

---

## 2. Shared Channel Behaviors

In the original Python implementation:
1. **Heartbeat / Typing:** Channels implement an async heartbeat that sends typing indicators to the user during long-running tasks or tool executions.
2. **Media Decoders:** Audio voice inputs are transcribed using Whisper before being forwarded to the agent loop. Image attachments are saved as bytes and sent to the multimodal model.
3. **Markdown Sanitization:** Channels like Slack and Feishu parse markdown blocks into their proprietary rich formatting models.
