//! One task per connected WebSocket.

use std::time::{Duration, Instant};

use actix_ws::{AggregatedMessage, CloseCode, CloseReason, Closed, Session};
use futures_util::StreamExt as _;
use tokio::{sync::mpsc, time::interval};

use crate::{
    db::{Store, dm_key, now_ms},
    hub::Hub,
    protocol::{Chat, ClientMsg, MAX_TEXT_LEN, ServerMsg, valid_name, valid_room},
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_FRAME_SIZE: usize = 64 * 1024;

pub async fn run(
    name: String,
    mut session: Session,
    stream: actix_ws::MessageStream,
    hub: Hub,
    store: Store,
) {
    let (outbox, mut inbox) = mpsc::unbounded_channel();

    if !hub.connect(&name, outbox) {
        let _ = session
            .text(
                ServerMsg::Error {
                    message: "That name is already online. Pick another one.",
                }
                .frame(),
            )
            .await;
        let _ = session
            .close(Some(CloseReason {
                code: CloseCode::Policy,
                description: Some("name taken".into()),
            }))
            .await;
        return;
    }

    log::info!("{name} connected");

    let welcome = ServerMsg::Welcome {
        me: &name,
        users: hub.online(),
        rooms: hub.rooms(),
    };
    if session.text(welcome.frame()).await.is_ok() {
        hub.broadcast(&ServerMsg::Presence { users: hub.online() }.frame());

        let mut stream = stream
            .max_frame_size(MAX_FRAME_SIZE)
            .aggregate_continuations()
            .max_continuation_size(MAX_FRAME_SIZE);
        let mut last_heartbeat = Instant::now();
        let mut heartbeat = interval(HEARTBEAT_INTERVAL);

        let reason = loop {
            tokio::select! {
                msg = stream.next() => match msg {
                    Some(Ok(AggregatedMessage::Text(text))) => {
                        last_heartbeat = Instant::now();
                        if handle(&name, &text, &hub, &store, &mut session).await.is_err() {
                            break None;
                        }
                    }
                    Some(Ok(AggregatedMessage::Ping(bytes))) => {
                        last_heartbeat = Instant::now();
                        let _ = session.pong(&bytes).await;
                    }
                    Some(Ok(AggregatedMessage::Pong(_))) => last_heartbeat = Instant::now(),
                    Some(Ok(AggregatedMessage::Binary(_))) => {}
                    Some(Ok(AggregatedMessage::Close(reason))) => break reason,
                    Some(Err(err)) => {
                        // Usually a tab closed without a close frame.
                        log::debug!("{name}: {err}");
                        break None;
                    }
                    None => break None,
                },

                Some(frame) = inbox.recv() => {
                    if session.text(frame).await.is_err() {
                        break None;
                    }
                }

                _ = heartbeat.tick() => {
                    if last_heartbeat.elapsed() > CLIENT_TIMEOUT
                        || session.ping(b"").await.is_err()
                    {
                        break None;
                    }
                }
            }
        };

        let _ = session.close(reason).await;
    }

    hub.disconnect(&name);
    hub.broadcast(&ServerMsg::Presence { users: hub.online() }.frame());
    log::info!("{name} disconnected");
}

async fn handle(
    name: &str,
    text: &str,
    hub: &Hub,
    store: &Store,
    session: &mut Session,
) -> Result<(), Closed> {
    let msg = match serde_json::from_str::<ClientMsg>(text) {
        Ok(msg) => msg,
        Err(_) => return reply_error(session, "Malformed message.").await,
    };

    match msg {
        ClientMsg::Join { room } => {
            if !valid_room(&room) {
                return reply_error(
                    session,
                    "Group names use letters, numbers, - and _ (max 32).",
                )
                .await;
            }
            if hub.join(&room, name) {
                store.save_room(&room);
                hub.broadcast(&ServerMsg::Rooms { rooms: hub.rooms() }.frame());
            }
            session.text(ServerMsg::Joined { room: &room }.frame()).await?;
        }

        ClientMsg::Leave { room } => hub.leave(&room, name),

        ClientMsg::Send { chat, text } => {
            let text = text.trim();
            if text.is_empty() {
                return Ok(());
            }
            if text.chars().count() > MAX_TEXT_LEN {
                return reply_error(session, "Message is too long.").await;
            }
            let ts = now_ms();

            match chat {
                Chat::Room(room) => {
                    if !hub.is_member(&room, name) {
                        return reply_error(session, "Join the group first.").await;
                    }
                    let frame = ServerMsg::Message {
                        chat: Chat::Room(room.clone()),
                        from: name,
                        text,
                        ts,
                    }
                    .frame();
                    hub.send_to_room(&room, &frame, None);
                    store.save_message("room", room, name, text, ts);
                }

                Chat::Dm(peer) => {
                    if !valid_name(&peer) {
                        return reply_error(session, "Unknown person.").await;
                    }
                    // Each side sees the conversation keyed by the other person.
                    let to_peer = ServerMsg::Message {
                        chat: Chat::Dm(name.to_owned()),
                        from: name,
                        text,
                        ts,
                    }
                    .frame();
                    hub.send_to_user(&peer, &to_peer);
                    if peer != name {
                        let to_me = ServerMsg::Message {
                            chat: Chat::Dm(peer.clone()),
                            from: name,
                            text,
                            ts,
                        }
                        .frame();
                        hub.send_to_user(name, &to_me);
                    }
                    store.save_message("dm", dm_key(name, &peer), name, text, ts);
                }
            }
        }

        ClientMsg::Typing { chat, active } => match chat {
            Chat::Room(room) => {
                if hub.is_member(&room, name) {
                    let frame = ServerMsg::Typing {
                        chat: Chat::Room(room.clone()),
                        from: name,
                        active,
                    }
                    .frame();
                    hub.send_to_room(&room, &frame, Some(name));
                }
            }
            Chat::Dm(peer) => {
                if peer != name {
                    let frame = ServerMsg::Typing {
                        chat: Chat::Dm(name.to_owned()),
                        from: name,
                        active,
                    }
                    .frame();
                    hub.send_to_user(&peer, &frame);
                }
            }
        },

        ClientMsg::History { chat } => {
            let (kind, target) = match &chat {
                Chat::Room(room) => {
                    if !hub.is_member(room, name) {
                        return reply_error(session, "Join the group first.").await;
                    }
                    ("room", room.clone())
                }
                Chat::Dm(peer) => ("dm", dm_key(name, peer)),
            };

            let messages = store.history(kind, &target).await.unwrap_or_else(|err| {
                log::error!("history for {kind}:{target}: {err}");
                Vec::new()
            });
            session
                .text(ServerMsg::History { chat, messages }.frame())
                .await?;
        }
    }

    Ok(())
}

async fn reply_error(session: &mut Session, message: &str) -> Result<(), Closed> {
    session.text(ServerMsg::Error { message }.frame()).await
}
