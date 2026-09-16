use std::{
    net::TcpListener,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use actix_web::{App, HttpServer, web};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{
    audio::AudioEngine,
    screen_capture,
    discovery::{self, Advertised, DiscoveredRoom},
    protocol::{ChatPayload, MediaControlMessage, PeerInfo, SignalingMessage},
    rooms::RoomManager,
    screen_capture::{CaptureTarget, ScreenCapturer, ShareSource},
    signaling::{SignalingClient, meeting_ws},
    webrtc_session::WebRtcMeshSession,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanIpInfo {
    pub local_ip: String,
    pub default_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingRoomInfo {
    pub room_id: String,
    pub peer_id: String,
    pub local_ip: String,
    pub join_address: String,
    pub is_host: bool,
    pub participants: Vec<PeerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingStatusResponse {
    pub in_meeting: bool,
    pub room_id: Option<String>,
    pub peer_id: Option<String>,
    pub is_host: bool,
    pub is_muted: bool,
    pub is_sharing_screen: bool,
    pub participants: Vec<PeerInfo>,
}

#[allow(dead_code)]
pub struct ActiveMeeting {
    pub room_id: String,
    pub peer_id: String,
    pub name: String,
    pub is_host: bool,
    pub join_address: String,
    pub webrtc_session: WebRtcMeshSession,
    pub audio_engine: Arc<Mutex<AudioEngine>>,
    pub screen_capturer: Arc<ScreenCapturer>,
    pub signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
    pub participants: Arc<Mutex<Vec<PeerInfo>>>,
    pub is_sharing_screen: Arc<Mutex<bool>>,
}

#[derive(Clone, Default)]
pub struct LanMeetingManager {
    pub room_manager: RoomManager,
    pub active_meeting: Arc<Mutex<Option<ActiveMeeting>>>,
    pub server_running_port: Arc<Mutex<Option<u16>>>,
    /// The room this machine currently advertises to the LAN, if any.
    pub advertised: Advertised,
    pub discovery_running: Arc<AtomicBool>,
}

/// Helper function to perform meeting cleanup
async fn do_leave_meeting(
    active_meeting_slot: &Arc<Mutex<Option<ActiveMeeting>>>,
    advertised: &Advertised,
    app_handle: &AppHandle,
) {
    if let Ok(mut room) = advertised.lock() {
        *room = None;
    }
    let meeting = active_meeting_slot.lock().unwrap().take();
    if let Some(meeting) = meeting {
        log::info!("Leaving LAN meeting: {}", meeting.room_id);

        let _ = meeting.signaling_tx.send(SignalingMessage::LeaveRoom {
            room_id: meeting.room_id.clone(),
            peer_id: meeting.peer_id.clone(),
        });

        meeting.screen_capturer.stop_capture();

        if let Ok(mut engine) = meeting.audio_engine.lock() {
            engine.stop();
        }

        meeting.webrtc_session.close_all().await;

        let _ = app_handle.emit("meeting://left", ());
    }
}

/// Detects and returns the local LAN IP address of the machine.
#[tauri::command]
pub fn get_lan_ip_info() -> Result<LanIpInfo, String> {
    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    Ok(LanIpInfo {
        local_ip: ip,
        default_port: 8080,
    })
}

/// Hosts a new LAN meeting room. Starts Actix Web signaling server if not already running.
#[tauri::command]
pub async fn create_lan_meeting(
    name: String,
    custom_port: Option<u16>,
    state: State<'_, LanMeetingManager>,
    app_handle: AppHandle,
) -> Result<MeetingRoomInfo, String> {
    let mgr = state.inner();
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }

    // Leave active meeting if any
    do_leave_meeting(&mgr.active_meeting, &mgr.advertised, &app_handle).await;

    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let room_id = Uuid::new_v4().simple().to_string()[..6].to_lowercase();
    let peer_id = Uuid::new_v4().to_string();

    let port = {
        let mut port_guard = mgr.server_running_port.lock().unwrap();
        if let Some(p) = *port_guard {
            p
        } else {
            let requested_port = custom_port.unwrap_or(8080);
            let listener = TcpListener::bind(("0.0.0.0", requested_port))
                .or_else(|_| TcpListener::bind(("0.0.0.0", 0)))
                .map_err(|e| format!("Failed to bind signaling server port: {e}"))?;

            let bound_port = listener
                .local_addr()
                .map_err(|e| e.to_string())?
                .port();

            let room_mgr = mgr.room_manager.clone();
            std::thread::spawn(move || {
                actix_web::rt::System::new().block_on(async move {
                    let _ = HttpServer::new(move || {
                        App::new()
                            .app_data(web::Data::new(room_mgr.clone()))
                            .route("/ws/meeting/{room_id}", web::get().to(meeting_ws))
                    })
                    .listen(listener)
                    .expect("Failed to listen on signaling socket")
                    .run()
                    .await;
                });
            });

            *port_guard = Some(bound_port);
            bound_port
        }
    };

    discovery::start_responder(mgr.advertised.clone(), mgr.discovery_running.clone());
    if let Ok(mut room) = mgr.advertised.lock() {
        *room = Some(DiscoveredRoom {
            room_id: room_id.clone(),
            host_name: name.clone(),
            host_ip: local_ip.clone(),
            port,
            participant_count: 1,
        });
    }

    let join_address = format!("ws://{local_ip}:{port}/ws/meeting/{room_id}");
    let local_connect_url = format!("ws://127.0.0.1:{port}/ws/meeting/{room_id}");

    let sig_client = SignalingClient::connect(&local_connect_url).await?;
    let sig_tx = sig_client.tx.clone();

    sig_tx
        .send(SignalingMessage::JoinRoom {
            room_id: room_id.clone(),
            peer_id: peer_id.clone(),
            name: name.clone(),
        })
        .map_err(|e| format!("Failed to send join-room: {e}"))?;

    let audio_engine = Arc::new(Mutex::new(AudioEngine::new()));
    let screen_capturer = Arc::new(ScreenCapturer::new());
    let participants = Arc::new(Mutex::new(vec![PeerInfo {
        peer_id: peer_id.clone(),
        name: name.clone(),
        is_host: true,
    }]));
    let is_sharing_screen = Arc::new(Mutex::new(false));

    let webrtc_session = WebRtcMeshSession::new(
        peer_id.clone(),
        name.clone(),
        sig_tx.clone(),
        audio_engine.clone(),
        app_handle.clone(),
    );

    if let Ok(mut engine) = audio_engine.lock() {
        let _ = engine.start_output();
    }

    {
        let webrtc_clone = webrtc_session.clone();
        let app_handle_clone = app_handle.clone();
        let pid_clone = peer_id.clone();
        let mut engine = audio_engine.lock().unwrap();
        let _ = engine.start_input(move |samples| {
            let webrtc = webrtc_clone.clone();
            let app = app_handle_clone.clone();
            let pid = pid_clone.clone();
            let samples_vec = samples.to_vec();

            let sum_sq: f32 = samples_vec.iter().map(|s| s * s).sum();
            let rms = (sum_sq / samples_vec.len().max(1) as f32).sqrt();
            let is_speaking = rms > 0.02;

            tauri::async_runtime::spawn(async move {
                webrtc.broadcast_audio_samples(&samples_vec).await;
                let _ = app.emit(
                    "meeting://speaking",
                    serde_json::json!({ "peer_id": pid, "is_speaking": is_speaking }),
                );
            });
        });
    }

    spawn_signaling_handler(
        sig_client.rx,
        webrtc_session.clone(),
        participants.clone(),
        mgr.advertised.clone(),
        app_handle.clone(),
    );

    let active = ActiveMeeting {
        room_id: room_id.clone(),
        peer_id: peer_id.clone(),
        name: name.clone(),
        is_host: true,
        join_address: join_address.clone(),
        webrtc_session,
        audio_engine,
        screen_capturer,
        signaling_tx: sig_tx,
        participants: participants.clone(),
        is_sharing_screen,
    };

    let initial_participants = participants.lock().unwrap().clone();
    *mgr.active_meeting.lock().unwrap() = Some(active);

    Ok(MeetingRoomInfo {
        room_id,
        peer_id,
        local_ip,
        join_address,
        is_host: true,
        participants: initial_participants,
    })
}

/// Lists meeting rooms currently advertised on the local network.
///
/// One broadcast, a short listening window, whatever answered. A room the user
/// is hosting on this machine answers too, so it is filtered out here.
#[tauri::command]
pub async fn discover_lan_meetings(
    state: State<'_, LanMeetingManager>,
) -> Result<Vec<DiscoveredRoom>, String> {
    let own_room = state
        .inner()
        .active_meeting
        .lock()
        .unwrap()
        .as_ref()
        .map(|meeting| meeting.room_id.clone());

    let rooms = tauri::async_runtime::spawn_blocking(discovery::scan)
        .await
        .map_err(|e| format!("Discovery task failed: {e}"))??;

    Ok(rooms
        .into_iter()
        .filter(|room| Some(&room.room_id) != own_room.as_ref())
        .collect())
}

/// Admits or turns away a guest waiting at the door. Host only.
#[tauri::command]
pub async fn respond_meeting_join_request(
    peer_id: String,
    accept: bool,
    state: State<'_, LanMeetingManager>,
) -> Result<(), String> {
    let signaling_tx = {
        let meeting_guard = state.inner().active_meeting.lock().unwrap();
        let meeting = meeting_guard
            .as_ref()
            .ok_or_else(|| "Not currently in a meeting".to_string())?;
        if !meeting.is_host {
            return Err("Only the host can admit people".to_string());
        }
        meeting.signaling_tx.clone()
    };

    signaling_tx
        .send(SignalingMessage::JoinDecision { peer_id, accept })
        .map_err(|e| format!("Failed to send the decision: {e}"))
}

/// Joins an existing LAN meeting room as a peer.
#[tauri::command]
pub async fn join_lan_meeting(
    join_url_or_ip: String,
    room_id: String,
    name: String,
    state: State<'_, LanMeetingManager>,
    app_handle: AppHandle,
) -> Result<MeetingRoomInfo, String> {
    let mgr = state.inner();
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    let room_id = room_id.trim().to_string();
    if room_id.is_empty() {
        return Err("Room ID cannot be empty".to_string());
    }

    let url = if join_url_or_ip.starts_with("ws://") || join_url_or_ip.starts_with("wss://") {
        join_url_or_ip.trim().to_string()
    } else {
        let host = join_url_or_ip.trim();
        let addr = if host.contains(':') {
            host.to_string()
        } else {
            format!("{host}:8080")
        };
        format!("ws://{addr}/ws/meeting/{room_id}")
    };

    do_leave_meeting(&mgr.active_meeting, &mgr.advertised, &app_handle).await;

    let peer_id = Uuid::new_v4().to_string();
    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let sig_client = SignalingClient::connect(&url).await?;
    let sig_tx = sig_client.tx.clone();

    sig_tx
        .send(SignalingMessage::JoinRoom {
            room_id: room_id.clone(),
            peer_id: peer_id.clone(),
            name: name.clone(),
        })
        .map_err(|e| format!("Failed to send join-room: {e}"))?;

    let audio_engine = Arc::new(Mutex::new(AudioEngine::new()));
    let screen_capturer = Arc::new(ScreenCapturer::new());
    let participants = Arc::new(Mutex::new(vec![PeerInfo {
        peer_id: peer_id.clone(),
        name: name.clone(),
        is_host: false,
    }]));
    let is_sharing_screen = Arc::new(Mutex::new(false));

    let webrtc_session = WebRtcMeshSession::new(
        peer_id.clone(),
        name.clone(),
        sig_tx.clone(),
        audio_engine.clone(),
        app_handle.clone(),
    );

    if let Ok(mut engine) = audio_engine.lock() {
        let _ = engine.start_output();
    }

    {
        let webrtc_clone = webrtc_session.clone();
        let app_handle_clone = app_handle.clone();
        let pid_clone = peer_id.clone();
        let mut engine = audio_engine.lock().unwrap();
        let _ = engine.start_input(move |samples| {
            let webrtc = webrtc_clone.clone();
            let app = app_handle_clone.clone();
            let pid = pid_clone.clone();
            let samples_vec = samples.to_vec();

            let sum_sq: f32 = samples_vec.iter().map(|s| s * s).sum();
            let rms = (sum_sq / samples_vec.len().max(1) as f32).sqrt();
            let is_speaking = rms > 0.02;

            tauri::async_runtime::spawn(async move {
                webrtc.broadcast_audio_samples(&samples_vec).await;
                let _ = app.emit(
                    "meeting://speaking",
                    serde_json::json!({ "peer_id": pid, "is_speaking": is_speaking }),
                );
            });
        });
    }

    spawn_signaling_handler(
        sig_client.rx,
        webrtc_session.clone(),
        participants.clone(),
        mgr.advertised.clone(),
        app_handle.clone(),
    );

    let active = ActiveMeeting {
        room_id: room_id.clone(),
        peer_id: peer_id.clone(),
        name: name.clone(),
        is_host: false,
        join_address: url.clone(),
        webrtc_session,
        audio_engine,
        screen_capturer,
        signaling_tx: sig_tx,
        participants: participants.clone(),
        is_sharing_screen,
    };

    let initial_participants = participants.lock().unwrap().clone();
    *mgr.active_meeting.lock().unwrap() = Some(active);

    Ok(MeetingRoomInfo {
        room_id,
        peer_id,
        local_ip,
        join_address: url,
        is_host: false,
        participants: initial_participants,
    })
}

/// Keeps the advertised participant count in step with the real room.
fn sync_advertised_count(advertised: &Advertised, count: usize) {
    if let Ok(mut room) = advertised.lock() {
        if let Some(room) = room.as_mut() {
            room.participant_count = count;
        }
    }
}

fn spawn_signaling_handler(
    mut rx: mpsc::UnboundedReceiver<SignalingMessage>,
    webrtc: WebRtcMeshSession,
    participants: Arc<Mutex<Vec<PeerInfo>>>,
    advertised: Advertised,
    app_handle: AppHandle,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                SignalingMessage::PeerJoined {
                    peer_id,
                    name,
                    existing_peers,
                } => {
                    log::info!("Peer joined: {peer_id} ({name})");

                    if !existing_peers.is_empty() {
                        let mut p_lock = participants.lock().unwrap();
                        for ep in &existing_peers {
                            if !p_lock.iter().any(|p| p.peer_id == ep.peer_id) {
                                p_lock.push(ep.clone());
                            }
                        }
                        sync_advertised_count(&advertised, p_lock.len());
                        let _ = app_handle.emit("meeting://participants-update", &*p_lock);
                        let _ = app_handle.emit("meeting://join-accepted", ());
                    } else {
                        {
                            let mut p_lock = participants.lock().unwrap();
                            if !p_lock.iter().any(|p| p.peer_id == peer_id) {
                                p_lock.push(PeerInfo {
                                    peer_id: peer_id.clone(),
                                    name: name.clone(),
                                    is_host: false,
                                });
                            }
                            sync_advertised_count(&advertised, p_lock.len());
                            let _ = app_handle.emit("meeting://participants-update", &*p_lock);
                        }

                        let webrtc_clone = webrtc.clone();
                        let target_pid = peer_id.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = webrtc_clone.initiate_peer_connection(&target_pid).await {
                                log::error!("Failed to initiate peer connection to {target_pid}: {e}");
                            }
                        });
                    }
                }
                SignalingMessage::Offer {
                    from_peer_id,
                    sdp,
                    ..
                } => {
                    let webrtc_clone = webrtc.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = webrtc_clone.handle_offer(&from_peer_id, &sdp).await {
                            log::error!("Failed to handle offer from {from_peer_id}: {e}");
                        }
                    });
                }
                SignalingMessage::Answer {
                    from_peer_id,
                    sdp,
                    ..
                } => {
                    let webrtc_clone = webrtc.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = webrtc_clone.handle_answer(&from_peer_id, &sdp).await {
                            log::error!("Failed to handle answer from {from_peer_id}: {e}");
                        }
                    });
                }
                SignalingMessage::IceCandidate {
                    from_peer_id,
                    candidate,
                    ..
                } => {
                    let webrtc_clone = webrtc.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = webrtc_clone
                            .handle_ice_candidate(&from_peer_id, &candidate)
                            .await
                        {
                            log::error!("Failed to handle ICE candidate from {from_peer_id}: {e}");
                        }
                    });
                }
                SignalingMessage::PeerLeft { peer_id } => {
                    log::info!("Peer left: {peer_id}");
                    {
                        let mut p_lock = participants.lock().unwrap();
                        p_lock.retain(|p| p.peer_id != peer_id);
                        sync_advertised_count(&advertised, p_lock.len());
                        let _ = app_handle.emit("meeting://participants-update", &*p_lock);
                    }
                    webrtc.remove_peer(&peer_id).await;
                    let _ = app_handle.emit("meeting://participant-left", &peer_id);
                }
                SignalingMessage::JoinRequested { peer_id, name } => {
                    log::info!("Join request from {name} ({peer_id})");
                    let _ = app_handle.emit(
                        "meeting://join-request",
                        serde_json::json!({ "peer_id": peer_id, "name": name }),
                    );
                }
                SignalingMessage::JoinPending { room_id } => {
                    let _ = app_handle.emit(
                        "meeting://join-pending",
                        serde_json::json!({ "room_id": room_id }),
                    );
                }
                SignalingMessage::JoinRejected { room_id } => {
                    log::info!("Join request rejected for room {room_id}");
                    let _ = app_handle.emit(
                        "meeting://join-rejected",
                        serde_json::json!({ "room_id": room_id }),
                    );
                }
                SignalingMessage::Error { message } => {
                    log::error!("Signaling error: {message}");
                    let _ = app_handle.emit("meeting://error", &message);
                }
                _ => {}
            }
        }
    });
}

/// Leaves the current meeting and releases all media resources.
#[tauri::command]
pub async fn leave_lan_meeting(
    state: State<'_, LanMeetingManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    do_leave_meeting(
        &state.inner().active_meeting,
        &state.inner().advertised,
        &app_handle,
    )
    .await;
    Ok(())
}

/// Toggles microphone mute/unmute.
#[tauri::command]
pub async fn toggle_meeting_mic(state: State<'_, LanMeetingManager>) -> Result<bool, String> {
    let mgr = state.inner();
    let meeting_guard = mgr.active_meeting.lock().unwrap();
    if let Some(meeting) = meeting_guard.as_ref() {
        let is_muted = {
            let engine = meeting.audio_engine.lock().unwrap();
            engine.toggle_mute()
        };

        let webrtc = meeting.webrtc_session.clone();
        let pid = meeting.peer_id.clone();
        tauri::async_runtime::spawn(async move {
            webrtc
                .broadcast_control(&MediaControlMessage::MicStatus {
                    peer_id: pid,
                    is_muted,
                })
                .await;
        });

        Ok(is_muted)
    } else {
        Err("Not currently in a meeting".to_string())
    }
}

/// Lists the monitors and windows the user can pick from when sharing.
#[tauri::command]
pub async fn list_share_sources() -> Result<Vec<ShareSource>, String> {
    // Capturing a preview per window is slow enough to keep off the UI thread.
    tauri::async_runtime::spawn_blocking(screen_capture::list_sources)
        .await
        .map_err(|e| format!("Failed to list share sources: {e}"))?
}

/// Toggles desktop screen sharing.
#[tauri::command]
pub async fn toggle_meeting_screen_share(
    source: Option<CaptureTarget>,
    state: State<'_, LanMeetingManager>,
    app_handle: AppHandle,
) -> Result<bool, String> {
    // No explicit pick means the primary monitor, as before the picker existed.
    let target = source.unwrap_or_default();
    let mgr = state.inner();
    let meeting_guard = mgr.active_meeting.lock().unwrap();
    if let Some(meeting) = meeting_guard.as_ref() {
        let mut is_sharing_lock = meeting.is_sharing_screen.lock().unwrap();
        let new_state = !*is_sharing_lock;
        *is_sharing_lock = new_state;

        let webrtc = meeting.webrtc_session.clone();
        let pid = meeting.peer_id.clone();
        let name = meeting.name.clone();

        if new_state {
            let capturer = meeting.screen_capturer.clone();
            let webrtc_clone = webrtc.clone();
            let app_handle_clone = app_handle.clone();
            let local_pid = pid.clone();

            capturer.start_capture(target, move |jpeg_bytes| {
                let webrtc = webrtc_clone.clone();
                let app = app_handle_clone.clone();
                let pid = local_pid.clone();

                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
                let payload = serde_json::json!({
                    "peer_id": pid,
                    "data_url": format!("data:image/jpeg;base64,{b64}")
                });
                let _ = app.emit("meeting://screen-frame", payload);

                tauri::async_runtime::spawn(async move {
                    webrtc.broadcast_screen_frame(&jpeg_bytes).await;
                });
            })?;

            let pid_ctrl = pid.clone();
            tauri::async_runtime::spawn(async move {
                webrtc
                    .broadcast_control(&MediaControlMessage::ScreenStart {
                        peer_id: pid_ctrl,
                        name,
                    })
                    .await;
            });
            let _ = app_handle.emit(
                "meeting://screen-share-started",
                serde_json::json!({ "peer_id": pid }),
            );
        } else {
            meeting.screen_capturer.stop_capture();
            let pid_ctrl = pid.clone();
            tauri::async_runtime::spawn(async move {
                webrtc
                    .broadcast_control(&MediaControlMessage::ScreenStop {
                        peer_id: pid_ctrl,
                    })
                    .await;
            });
            let _ = app_handle.emit(
                "meeting://screen-share-stopped",
                serde_json::json!({ "peer_id": pid }),
            );
        }

        Ok(new_state)
    } else {
        Err("Not currently in a meeting".to_string())
    }
}

/// Sends a chat message over WebRTC data channel to all connected peers.
#[tauri::command]
pub async fn send_meeting_chat(
    text: String,
    state: State<'_, LanMeetingManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }

    let mgr = state.inner();
    let meeting_guard = mgr.active_meeting.lock().unwrap();
    if let Some(meeting) = meeting_guard.as_ref() {
        let chat = ChatPayload {
            id: Uuid::new_v4().to_string(),
            sender_id: meeting.peer_id.clone(),
            sender_name: meeting.name.clone(),
            text,
            timestamp: chrono_or_now(),
        };

        let _ = app_handle.emit("meeting://chat-message", &chat);

        let webrtc = meeting.webrtc_session.clone();
        tauri::async_runtime::spawn(async move {
            webrtc.broadcast_chat(&chat).await;
        });

        Ok(())
    } else {
        Err("Not currently in a meeting".to_string())
    }
}

/// Returns current meeting state and participant list.
#[tauri::command]
pub fn get_meeting_status(
    state: State<'_, LanMeetingManager>,
) -> Result<MeetingStatusResponse, String> {
    let mgr = state.inner();
    let meeting_guard = mgr.active_meeting.lock().unwrap();
    if let Some(meeting) = meeting_guard.as_ref() {
        let is_muted = meeting
            .audio_engine
            .lock()
            .map(|e| e.is_muted())
            .unwrap_or(false);
        let is_sharing = *meeting.is_sharing_screen.lock().unwrap();
        let participants = meeting.participants.lock().unwrap().clone();

        Ok(MeetingStatusResponse {
            in_meeting: true,
            room_id: Some(meeting.room_id.clone()),
            peer_id: Some(meeting.peer_id.clone()),
            is_host: meeting.is_host,
            is_muted,
            is_sharing_screen: is_sharing,
            participants,
        })
    } else {
        Ok(MeetingStatusResponse {
            in_meeting: false,
            room_id: None,
            peer_id: None,
            is_host: false,
            is_muted: false,
            is_sharing_screen: false,
            participants: vec![],
        })
    }
}

fn chrono_or_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
