//! Runtime, slash-command, and managed-server WebSocket response builders.

use serde_json::Value;

pub(crate) fn runtime_inventory_event(
    config: &crate::config::schema::Config,
    tools: Option<&crate::tools::ToolRegistry>,
) -> Value {
    let inventory = crate::core::inventory::build_runtime_inventory(config, tools);
    super::super::protocol::runtime_inventory(inventory)
}

pub(crate) fn slash_commands_event() -> Value {
    let commands = crate::channels::cli::render::SLASH_COMMANDS
        .iter()
        .map(|(cmd, desc)| super::super::protocol::slash_command(*cmd, *desc))
        .collect::<Vec<_>>();
    super::super::protocol::slash_commands(commands)
}

pub(crate) fn status_event() -> Value {
    let (loaded, failed, total) = crate::channels::cli::mcp::get_mcp_stats();
    super::super::protocol::gateway_status(loaded, failed, total)
}

pub(crate) async fn servers_list_event(config: &crate::config::schema::Config) -> Value {
    let servers = crate::shutdown::list_registered_children();
    let list = servers
        .into_iter()
        .map(|server| {
            super::super::protocol::managed_server(
                server.id,
                server.pid,
                server.kind,
                server.command,
            )
        })
        .collect::<Vec<_>>();

    let channel_status = |name: &str, enabled: bool, token_configured: bool| {
        super::super::protocol::channel_status(name, enabled, token_configured)
    };
    let telegram = config.channels.telegram.as_ref();
    let discord = config.channels.discord.as_ref();
    let whatsapp = config.channels.whatsapp.as_ref();
    let channels = vec![
        channel_status(
            "telegram",
            telegram.is_some_and(|channel| channel.enabled),
            telegram.is_some_and(|channel| !channel.bot_token.is_empty()),
        ),
        channel_status(
            "discord",
            discord.is_some_and(|channel| channel.enabled),
            discord.is_some_and(|channel| !channel.bot_token.is_empty()),
        ),
        channel_status(
            "whatsapp",
            whatsapp.is_some_and(|channel| channel.enabled),
            whatsapp.is_some_and(|channel| !channel.api_key.is_empty()),
        ),
    ];

    super::super::protocol::servers_list(list, channels)
}

pub(crate) fn stop_server_event(envelope: &Value) -> Value {
    let target = envelope
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("");
    let result = match crate::shutdown::stop_registered_child(target) {
        Ok(count) => format!("Stopped {count} server(s) successfully."),
        Err(error) => format!("Failed to stop server: {error}"),
    };
    super::super::protocol::server_stopped(target, result)
}
