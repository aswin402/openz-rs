use super::*;

#[test]
fn discord_uses_shared_stop_command_detection() {
    assert!(crate::channels::is_stop_command("/stop"));
    assert!(!crate::channels::is_stop_command("stop"));
}

#[test]
fn test_deserialize_gateway_hello() {
    let hello_json = r#"{
        "op": 10,
        "d": {
            "heartbeat_interval": 41250
        }
    }"#;
    let parsed: GatewayMessage = serde_json::from_str(hello_json).unwrap();
    assert_eq!(parsed.op, 10);
    let hello: HelloPayload = serde_json::from_value(parsed.d.unwrap()).unwrap();
    assert_eq!(hello.heartbeat_interval, 41250);
}

#[test]
fn test_deserialize_gateway_message_create() {
    let msg_json = r#"{
        "op": 0,
        "t": "MESSAGE_CREATE",
        "d": {
            "channel_id": "123456",
            "content": "hello openz",
            "author": {
                "id": "789",
                "username": "testuser",
                "bot": false
            }
        }
    }"#;
    let parsed: GatewayMessage = serde_json::from_str(msg_json).unwrap();
    assert_eq!(parsed.op, 0);
    assert_eq!(parsed.t.unwrap(), "MESSAGE_CREATE");
    let payload: MessageCreatePayload = serde_json::from_value(parsed.d.unwrap()).unwrap();
    assert_eq!(payload.channel_id, "123456");
    assert_eq!(payload.content, "hello openz");
    assert_eq!(payload.author.bot, Some(false));
}
