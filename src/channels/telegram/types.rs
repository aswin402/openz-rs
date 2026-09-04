use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) struct TelegramUpdate {
    pub(crate) update_id: i64,
    pub(crate) message: Option<TelegramMessage>,
    pub(crate) callback_query: Option<TelegramCallbackQuery>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct TelegramMessage {
    pub(crate) chat: TelegramChat,
    pub(crate) text: Option<String>,
    pub(crate) message_id: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct TelegramChat {
    pub(crate) id: i64,
}

#[derive(Deserialize, Debug)]
pub(crate) struct TelegramCallbackQuery {
    pub(crate) id: String,
    pub(crate) data: Option<String>,
    pub(crate) message: Option<TelegramMessage>,
}

#[derive(Deserialize)]
pub(crate) struct TelegramApiResponse {
    pub(crate) ok: bool,
    pub(crate) description: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct UpdatesResponse {
    pub(crate) ok: bool,
    pub(crate) result: Vec<TelegramUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TelegramCommandAction {
    Stop,
    RemoteMode,
    None,
}
