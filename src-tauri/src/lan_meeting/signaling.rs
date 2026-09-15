use std::time::{Duration, Instant};

use actix_web::{Error, HttpRequest, HttpResponse, rt, web};
use actix_ws::{AggregatedMessage, Session};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

use super::protocol::SignalingMessage;
use super::rooms::RoomManager;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_FRAME_SIZE: usize = 256 * 1024; // 256 KB for SDP and candidates

/// Actix Web HTTP handler for WebSocket signaling
pub async fn meeting_ws(
    req: HttpRequest,
    body: web::Payload,
    path: web::Path<String>,
    room_manager: web::Data<RoomManager>,
) -> Result<HttpResponse, Error> {
    let room_id = path.into_inner();
    let (response, session, stream) = actix_ws::handle(&req, body)?;

    rt::spawn(run_server_session(
        room_id,
        session,
        stream,
        room_manager.get_ref().clone(),
    ));

    Ok(response)
}

async fn run_server_session(
    initial_room_id: String,
    mut session: Session,
    stream: actix_ws::MessageStream,
    rooms: RoomManager,
) {
    let (tx, mut rx) = mpsc::unbounded_channel::<SignalingMessage>();
    let mut joined_peer: Option<(String, String)> = None; // (room_id, peer_id)

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
                    match serde_json::from_str::<SignalingMessage>(&text) {
                        Ok(sig_msg) => {
                            match sig_msg {
                                SignalingMessage::JoinRoom { room_id, peer_id, name } => {
                                    joined_peer = Some((room_id.clone(), peer_id.clone()));
                                    match rooms.join_peer(&room_id, peer_id.clone(), name.clone(), tx.clone()).await {
                                        Ok((existing_peers, existing_senders)) => {
                                            // Send confirmation to the joining peer with list of existing peers
                                            let welcome = SignalingMessage::PeerJoined {
                                                peer_id: peer_id.clone(),
                                                name: name.clone(),
                                                existing_peers,
                                            };
                                            let _ = tx.send(welcome);

                                            // Notify all existing peers that a new peer has joined
                                            let notification = SignalingMessage::PeerJoined {
                                                peer_id: peer_id.clone(),
                                                name: name.clone(),
                                                existing_peers: vec![],
                                            };
                                            for sender in existing_senders {
                                                let _ = sender.send(notification.clone());
                                            }
                                        }
                                        Err(err_str) => {
                                            let err_msg = SignalingMessage::Error { message: err_str };
                                            let _ = tx.send(err_msg);
                                            break None;
                                        }
                                    }
                                }
                                SignalingMessage::Offer { ref to_peer_id, .. } => {
                                    let to_id = to_peer_id.clone();
                                    let target_room = joined_peer.as_ref().map(|(r, _)| r.as_str()).unwrap_or(&initial_room_id);
                                    rooms.route_to_peer(target_room, &to_id, sig_msg).await;
                                }
                                SignalingMessage::Answer { ref to_peer_id, .. } => {
                                    let to_id = to_peer_id.clone();
                                    let target_room = joined_peer.as_ref().map(|(r, _)| r.as_str()).unwrap_or(&initial_room_id);
                                    rooms.route_to_peer(target_room, &to_id, sig_msg).await;
                                }
                                SignalingMessage::IceCandidate { ref to_peer_id, .. } => {
                                    let to_id = to_peer_id.clone();
                                    let target_room = joined_peer.as_ref().map(|(r, _)| r.as_str()).unwrap_or(&initial_room_id);
                                    rooms.route_to_peer(target_room, &to_id, sig_msg).await;
                                }
                                SignalingMessage::LeaveRoom { room_id, peer_id } => {
                                    let senders = rooms.leave_peer(&room_id, &peer_id).await;
                                    let notification = SignalingMessage::PeerLeft { peer_id };
                                    for s in senders {
                                        let _ = s.send(notification.clone());
                                    }
                                    break None;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            log::warn!("Invalid signaling message: {e}");
                        }
                    }
                }
                Some(Ok(AggregatedMessage::Ping(bytes))) => {
                    last_heartbeat = Instant::now();
                    let _ = session.pong(&bytes).await;
                }
                Some(Ok(AggregatedMessage::Pong(_))) => {
                    last_heartbeat = Instant::now();
                }
                Some(Ok(AggregatedMessage::Binary(_))) => {}
                Some(Ok(AggregatedMessage::Close(reason))) => break reason,
                Some(Err(err)) => {
                    log::debug!("WebSocket error: {err}");
                    break None;
                }
                None => break None,
            },

            Some(sig_msg) = rx.recv() => {
                if let Ok(json) = serde_json::to_string(&sig_msg) {
                    if session.text(json).await.is_err() {
                        break None;
                    }
                }
            }

            _ = heartbeat.tick() => {
                if last_heartbeat.elapsed() > CLIENT_TIMEOUT || session.ping(b"").await.is_err() {
                    break None;
                }
            }
        }
    };

    let _ = session.close(reason).await;

    // Clean up peer if joined
    if let Some((room_id, peer_id)) = joined_peer {
        let senders = rooms.leave_peer(&room_id, &peer_id).await;
        let notification = SignalingMessage::PeerLeft { peer_id };
        for s in senders {
            let _ = s.send(notification.clone());
        }
    }
}

/// Client connector for connecting to a local or remote signaling server.
pub struct SignalingClient {
    pub tx: mpsc::UnboundedSender<SignalingMessage>,
    pub rx: mpsc::UnboundedReceiver<SignalingMessage>,
}

impl SignalingClient {
    pub async fn connect(url: &str) -> Result<Self, String> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| format!("Failed to connect to signaling server at {url}: {e}"))?;

        let (mut write, mut read) = ws_stream.split();

        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<SignalingMessage>();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<SignalingMessage>();

        // Outbound forwarder
        tokio::spawn(async move {
            while let Some(msg) = outbound_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if write.send(WsMessage::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            let _ = write.close().await;
        });

        // Inbound receiver
        tokio::spawn(async move {
            while let Some(res) = read.next().await {
                match res {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(sig_msg) = serde_json::from_str::<SignalingMessage>(&text) {
                            if inbound_tx.send(sig_msg).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(WsMessage::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        Ok(Self {
            tx: outbound_tx,
            rx: inbound_rx,
        })
    }
}
