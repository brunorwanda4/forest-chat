//! JSON messages exchanged over the WebSocket.

use bytestring::ByteString;
use serde::{Deserialize, Serialize};

use crate::db::StoredMessage;

pub const MAX_NAME_LEN: usize = 24;
pub const MAX_ROOM_LEN: usize = 32;
pub const MAX_TEXT_LEN: usize = 2000;

/// A conversation: a group room or a direct chat with one person.
///
/// Serialized as `{"kind": "room", "id": "general"}` or `{"kind": "dm", "id": "alice"}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Chat {
    Room(String),
    Dm(String),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Join { room: String },
    Leave { room: String },
    Send { chat: Chat, text: String },
    Typing { chat: Chat, active: bool },
    History { chat: Chat },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg<'a> {
    Welcome {
        me: &'a str,
        users: Vec<String>,
        rooms: Vec<String>,
    },
    Presence {
        users: Vec<String>,
    },
    Rooms {
        rooms: Vec<String>,
    },
    Joined {
        room: &'a str,
    },
    Message {
        chat: Chat,
        from: &'a str,
        text: &'a str,
        ts: i64,
    },
    Typing {
        chat: Chat,
        from: &'a str,
        active: bool,
    },
    History {
        chat: Chat,
        messages: Vec<StoredMessage>,
    },
    Error {
        message: &'a str,
    },
}

impl ServerMsg<'_> {
    pub fn frame(&self) -> ByteString {
        serde_json::to_string(self)
            .expect("server messages always serialize")
            .into()
    }
}

fn valid_ident(s: &str, max_len: usize) -> bool {
    !s.is_empty()
        && s.chars().count() <= max_len
        && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

pub fn valid_name(name: &str) -> bool {
    valid_ident(name, MAX_NAME_LEN)
}

pub fn valid_room(room: &str) -> bool {
    valid_ident(room, MAX_ROOM_LEN)
}
