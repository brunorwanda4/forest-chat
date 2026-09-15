use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use webrtc::{
    api::{API, APIBuilder},
    data_channel::{RTCDataChannel, data_channel_message::DataChannelMessage},
    ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
    peer_connection::{
        RTCPeerConnection,
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
};

use super::{
    audio::AudioEngine,
    protocol::{ChatPayload, MediaControlMessage, SignalingMessage},
};

pub struct PeerConnectionEntry {
    pub pc: Arc<RTCPeerConnection>,
    pub chat_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    pub screen_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    pub audio_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct WebRtcMeshSession {
    api: Arc<API>,
    local_peer_id: String,
    local_name: String,
    peers: Arc<Mutex<HashMap<String, PeerConnectionEntry>>>,
    signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
    audio_engine: Arc<Mutex<AudioEngine>>,
    app_handle: AppHandle,
}

impl WebRtcMeshSession {
    pub fn new(
        local_peer_id: String,
        local_name: String,
        signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
        audio_engine: Arc<Mutex<AudioEngine>>,
        app_handle: AppHandle,
    ) -> Self {
        let api = Arc::new(APIBuilder::new().build());
        Self {
            api,
            local_peer_id,
            local_name,
            peers: Arc::new(Mutex::new(HashMap::new())),
            signaling_tx,
            audio_engine,
            app_handle,
        }
    }

    fn rtc_config() -> RTCConfiguration {
        // Pure LAN P2P: No external STUN/TURN servers needed
        RTCConfiguration {
            ice_servers: vec![],
            ..Default::default()
        }
    }

    /// Initiates a WebRTC peer connection to a remote peer (creates offer and data channels).
    pub async fn initiate_peer_connection(&self, remote_peer_id: &str) -> Result<(), String> {
        let pc = Arc::new(
            self.api
                .new_peer_connection(Self::rtc_config())
                .await
                .map_err(|e| format!("Failed to create PeerConnection: {e}"))?,
        );

        let chat_channel = Arc::new(Mutex::new(None));
        let screen_channel = Arc::new(Mutex::new(None));
        let audio_channel = Arc::new(Mutex::new(None));

        // Create Chat DataChannel
        let chat_dc = pc
            .create_data_channel("chat", None)
            .await
            .map_err(|e| format!("Failed to create chat data channel: {e}"))?;
        self.setup_chat_channel(chat_dc.clone(), remote_peer_id);
        *chat_channel.lock().unwrap() = Some(chat_dc);

        // Create Screen DataChannel
        let screen_dc = pc
            .create_data_channel("screen", None)
            .await
            .map_err(|e| format!("Failed to create screen data channel: {e}"))?;
        self.setup_screen_channel(screen_dc.clone(), remote_peer_id);
        *screen_channel.lock().unwrap() = Some(screen_dc);

        // Create Audio DataChannel
        let audio_dc = pc
            .create_data_channel("audio", None)
            .await
            .map_err(|e| format!("Failed to create audio data channel: {e}"))?;
        self.setup_audio_channel(audio_dc.clone());
        *audio_channel.lock().unwrap() = Some(audio_dc);

        self.setup_ice_handling(pc.clone(), remote_peer_id);

        let entry = PeerConnectionEntry {
            pc: pc.clone(),
            chat_channel,
            screen_channel,
            audio_channel,
        };
        self.peers
            .lock()
            .unwrap()
            .insert(remote_peer_id.to_string(), entry);

        // Create and set local offer
        let offer = pc
            .create_offer(None)
            .await
            .map_err(|e| format!("Failed to create offer: {e}"))?;
        pc.set_local_description(offer.clone())
            .await
            .map_err(|e| format!("Failed to set local description: {e}"))?;

        let _ = self.signaling_tx.send(SignalingMessage::Offer {
            from_peer_id: self.local_peer_id.clone(),
            to_peer_id: remote_peer_id.to_string(),
            sdp: offer.sdp,
        });

        Ok(())
    }

    /// Handles an incoming offer from a remote peer (creates answer and awaits data channels).
    pub async fn handle_offer(&self, from_peer_id: &str, sdp: &str) -> Result<(), String> {
        let pc = Arc::new(
            self.api
                .new_peer_connection(Self::rtc_config())
                .await
                .map_err(|e| format!("Failed to create PeerConnection: {e}"))?,
        );

        let chat_channel = Arc::new(Mutex::new(None));
        let screen_channel = Arc::new(Mutex::new(None));
        let audio_channel = Arc::new(Mutex::new(None));

        // Listen for incoming data channels
        let chat_slot = chat_channel.clone();
        let screen_slot = screen_channel.clone();
        let audio_slot = audio_channel.clone();
        let self_clone = self.clone();
        let from_pid = from_peer_id.to_string();

        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let label = dc.label().to_string();
            match label.as_str() {
                "chat" => {
                    self_clone.setup_chat_channel(dc.clone(), &from_pid);
                    *chat_slot.lock().unwrap() = Some(dc);
                }
                "screen" => {
                    self_clone.setup_screen_channel(dc.clone(), &from_pid);
                    *screen_slot.lock().unwrap() = Some(dc);
                }
                "audio" => {
                    self_clone.setup_audio_channel(dc.clone());
                    *audio_slot.lock().unwrap() = Some(dc);
                }
                _ => {}
            }
            Box::pin(async {})
        }));

        self.setup_ice_handling(pc.clone(), from_peer_id);

        let entry = PeerConnectionEntry {
            pc: pc.clone(),
            chat_channel,
            screen_channel,
            audio_channel,
        };
        self.peers
            .lock()
            .unwrap()
            .insert(from_peer_id.to_string(), entry);

        let desc = RTCSessionDescription::offer(sdp.to_string())
            .map_err(|e| format!("Failed to parse offer SDP: {e}"))?;
        pc.set_remote_description(desc)
            .await
            .map_err(|e| format!("Failed to set remote description: {e}"))?;

        let answer = pc
            .create_answer(None)
            .await
            .map_err(|e| format!("Failed to create answer: {e}"))?;
        pc.set_local_description(answer.clone())
            .await
            .map_err(|e| format!("Failed to set local answer description: {e}"))?;

        let _ = self.signaling_tx.send(SignalingMessage::Answer {
            from_peer_id: self.local_peer_id.clone(),
            to_peer_id: from_peer_id.to_string(),
            sdp: answer.sdp,
        });

        Ok(())
    }

    /// Handles an incoming answer from a remote peer.
    pub async fn handle_answer(&self, from_peer_id: &str, sdp: &str) -> Result<(), String> {
        let pc = {
            let peers = self.peers.lock().unwrap();
            peers
                .get(from_peer_id)
                .map(|p| p.pc.clone())
                .ok_or_else(|| format!("Unknown peer {from_peer_id}"))?
        };

        let desc = RTCSessionDescription::answer(sdp.to_string())
            .map_err(|e| format!("Failed to parse answer SDP: {e}"))?;
        pc.set_remote_description(desc)
            .await
            .map_err(|e| format!("Failed to set remote answer description: {e}"))?;

        Ok(())
    }

    /// Handles an incoming ICE candidate from a remote peer.
    pub async fn handle_ice_candidate(
        &self,
        from_peer_id: &str,
        candidate_json: &str,
    ) -> Result<(), String> {
        let pc = {
            let peers = self.peers.lock().unwrap();
            peers
                .get(from_peer_id)
                .map(|p| p.pc.clone())
                .ok_or_else(|| format!("Unknown peer {from_peer_id}"))?
        };

        let init: RTCIceCandidateInit = serde_json::from_str(candidate_json)
            .map_err(|e| format!("Failed to parse ICE candidate JSON: {e}"))?;
        pc.add_ice_candidate(init)
            .await
            .map_err(|e| format!("Failed to add ICE candidate: {e}"))?;

        Ok(())
    }

    fn setup_ice_handling(&self, pc: Arc<RTCPeerConnection>, remote_peer_id: &str) {
        let sig_tx = self.signaling_tx.clone();
        let local_pid = self.local_peer_id.clone();
        let target_pid = remote_peer_id.to_string();

        pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
            if let Some(c) = candidate {
                if let Ok(c_json) = c.to_json() {
                    if let Ok(cand_str) = serde_json::to_string(&c_json) {
                        let _ = sig_tx.send(SignalingMessage::IceCandidate {
                            from_peer_id: local_pid.clone(),
                            to_peer_id: target_pid.clone(),
                            candidate: cand_str,
                        });
                    }
                }
            }
            Box::pin(async {})
        }));

        let app_handle = self.app_handle.clone();
        let remote_pid = remote_peer_id.to_string();
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            log::info!("Peer {remote_pid} state: {s}");
            if s == RTCPeerConnectionState::Failed || s == RTCPeerConnectionState::Closed {
                let _ = app_handle.emit("meeting://peer-disconnected", &remote_pid);
            }
            Box::pin(async {})
        }));
    }

    fn setup_chat_channel(&self, dc: Arc<RTCDataChannel>, _remote_peer_id: &str) {
        let app_handle = self.app_handle.clone();
        dc.on_message(Box::new(move |msg: DataChannelMessage| {
            if let Ok(text) = std::str::from_utf8(&msg.data) {
                if let Ok(chat) = serde_json::from_str::<ChatPayload>(text) {
                    let _ = app_handle.emit("meeting://chat-message", &chat);
                } else if let Ok(ctrl) = serde_json::from_str::<MediaControlMessage>(text) {
                    match ctrl {
                        MediaControlMessage::ScreenStart { peer_id, name } => {
                            let _ = app_handle.emit(
                                "meeting://screen-share-started",
                                serde_json::json!({ "peer_id": peer_id, "name": name }),
                            );
                        }
                        MediaControlMessage::ScreenStop { peer_id } => {
                            let _ = app_handle.emit(
                                "meeting://screen-share-stopped",
                                serde_json::json!({ "peer_id": peer_id }),
                            );
                        }
                        MediaControlMessage::MicStatus { peer_id, is_muted } => {
                            let _ = app_handle.emit(
                                "meeting://mic-status",
                                serde_json::json!({ "peer_id": peer_id, "is_muted": is_muted }),
                            );
                        }
                    }
                }
            }
            Box::pin(async {})
        }));
    }

    fn setup_screen_channel(&self, dc: Arc<RTCDataChannel>, remote_peer_id: &str) {
        let app_handle = self.app_handle.clone();
        let rpid = remote_peer_id.to_string();
        dc.on_message(Box::new(move |msg: DataChannelMessage| {
            // Screen frame is raw JPEG bytes
            // Convert to base64 data URL for instant zero-dependency HTML canvas rendering
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&msg.data);
            let payload = serde_json::json!({
                "peer_id": rpid,
                "data_url": format!("data:image/jpeg;base64,{b64}")
            });
            let _ = app_handle.emit("meeting://screen-frame", payload);
            Box::pin(async {})
        }));
    }

    fn setup_audio_channel(&self, dc: Arc<RTCDataChannel>) {
        let audio_engine = self.audio_engine.clone();
        dc.on_message(Box::new(move |msg: DataChannelMessage| {
            if msg.data.len() >= 4 && msg.data.len() % 4 == 0 {
                let count = msg.data.len() / 4;
                let mut samples = Vec::with_capacity(count);
                for chunk in msg.data.chunks_exact(4) {
                    let bytes: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                    samples.push(f32::from_le_bytes(bytes));
                }
                if let Ok(engine) = audio_engine.lock() {
                    engine.push_incoming_audio(&samples);
                }
            }
            Box::pin(async {})
        }));
    }

    /// Broadcasts a chat message to all connected peers over their WebRTC chat data channel.
    pub async fn broadcast_chat(&self, chat: &ChatPayload) {
        if let Ok(json) = serde_json::to_string(chat) {
            let bytes = bytestring::ByteString::from(json);
            let channels: Vec<Arc<RTCDataChannel>> = {
                let peers = self.peers.lock().unwrap();
                peers
                    .values()
                    .filter_map(|e| e.chat_channel.lock().unwrap().clone())
                    .collect()
            };
            for dc in channels {
                let _ = dc.send_text(bytes.to_string()).await;
            }
        }
    }

    /// Broadcasts a media control message (screen start/stop, mic status) over WebRTC chat data channel.
    pub async fn broadcast_control(&self, ctrl: &MediaControlMessage) {
        if let Ok(json) = serde_json::to_string(ctrl) {
            let bytes = bytestring::ByteString::from(json);
            let channels: Vec<Arc<RTCDataChannel>> = {
                let peers = self.peers.lock().unwrap();
                peers
                    .values()
                    .filter_map(|e| e.chat_channel.lock().unwrap().clone())
                    .collect()
            };
            for dc in channels {
                let _ = dc.send_text(bytes.to_string()).await;
            }
        }
    }

    /// Broadcasts a screen capture frame (JPEG bytes) to all connected peers over their screen data channel.
    pub async fn broadcast_screen_frame(&self, frame: &[u8]) {
        use bytes::Bytes;
        let b = Bytes::copy_from_slice(frame);
        let channels: Vec<Arc<RTCDataChannel>> = {
            let peers = self.peers.lock().unwrap();
            peers
                .values()
                .filter_map(|e| e.screen_channel.lock().unwrap().clone())
                .collect()
        };
        for dc in channels {
            let _ = dc.send(&b).await;
        }
    }

    /// Broadcasts captured audio samples to all connected peers over their audio data channel.
    pub async fn broadcast_audio_samples(&self, samples: &[f32]) {
        use bytes::Bytes;
        let mut byte_buf = Vec::with_capacity(samples.len() * 4);
        for &sample in samples {
            byte_buf.extend_from_slice(&sample.to_le_bytes());
        }
        let b = Bytes::from(byte_buf);
        let channels: Vec<Arc<RTCDataChannel>> = {
            let peers = self.peers.lock().unwrap();
            peers
                .values()
                .filter_map(|e| e.audio_channel.lock().unwrap().clone())
                .collect()
        };
        for dc in channels {
            let _ = dc.send(&b).await;
        }
    }

    /// Closes and cleans up peer connection for a specific peer.
    pub async fn remove_peer(&self, peer_id: &str) {
        let entry = {
            let mut peers = self.peers.lock().unwrap();
            peers.remove(peer_id)
        };
        if let Some(entry) = entry {
            let _ = entry.pc.close().await;
        }
    }

    /// Closes all peer connections and clears peer list.
    pub async fn close_all(&self) {
        let entries: Vec<PeerConnectionEntry> = {
            let mut p = self.peers.lock().unwrap();
            p.drain().map(|(_, entry)| entry).collect()
        };
        for entry in entries {
            let _ = entry.pc.close().await;
        }
    }
}
