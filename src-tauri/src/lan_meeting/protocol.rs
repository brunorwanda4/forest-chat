use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerInfo {
    pub peer_id: String,
    pub name: String,
    pub is_host: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    #[serde(rename = "join-room")]
    JoinRoom {
        room_id: String,
        peer_id: String,
        name: String,
    },
    #[serde(rename = "peer-joined")]
    PeerJoined {
        peer_id: String,
        name: String,
        existing_peers: Vec<PeerInfo>,
    },
    #[serde(rename = "offer")]
    Offer {
        from_peer_id: String,
        to_peer_id: String,
        sdp: String,
    },
    #[serde(rename = "answer")]
    Answer {
        from_peer_id: String,
        to_peer_id: String,
        sdp: String,
    },
    #[serde(rename = "ice-candidate")]
    IceCandidate {
        from_peer_id: String,
        to_peer_id: String,
        candidate: String,
    },
    /// Sent to the host when a guest is waiting to be let in.
    #[serde(rename = "join-requested")]
    JoinRequested {
        peer_id: String,
        name: String,
    },
    /// Sent to the guest: the host has been asked, hold on.
    #[serde(rename = "join-pending")]
    JoinPending {
        room_id: String,
    },
    /// The host's verdict on a waiting guest.
    #[serde(rename = "join-decision")]
    JoinDecision {
        peer_id: String,
        accept: bool,
    },
    /// Sent to the guest when the host says no.
    #[serde(rename = "join-rejected")]
    JoinRejected {
        room_id: String,
    },
    #[serde(rename = "peer-left")]
    PeerLeft {
        peer_id: String,
    },
    #[serde(rename = "leave-room")]
    LeaveRoom {
        room_id: String,
        peer_id: String,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPayload {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MediaControlMessage {
    #[serde(rename = "screen-start")]
    ScreenStart { peer_id: String, name: String },
    #[serde(rename = "screen-stop")]
    ScreenStop { peer_id: String },
    #[serde(rename = "mic-status")]
    MicStatus { peer_id: String, is_muted: bool },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_messages_roundtrip() {
        // join-room
        let join = SignalingMessage::JoinRoom {
            room_id: "room123".into(),
            peer_id: "peer1".into(),
            name: "Alice".into(),
        };
        let json = serde_json::to_string(&join).unwrap();
        assert!(json.contains("\"type\":\"join-room\""));
        assert!(json.contains("\"room_id\":\"room123\""));

        // peer-joined
        let joined = SignalingMessage::PeerJoined {
            peer_id: "peer2".into(),
            name: "Bob".into(),
            existing_peers: vec![PeerInfo {
                peer_id: "peer1".into(),
                name: "Alice".into(),
                is_host: true,
            }],
        };
        let json = serde_json::to_string(&joined).unwrap();
        assert!(json.contains("\"type\":\"peer-joined\""));

        // offer
        let offer = SignalingMessage::Offer {
            from_peer_id: "peer1".into(),
            to_peer_id: "peer2".into(),
            sdp: "v=0...".into(),
        };
        let json = serde_json::to_string(&offer).unwrap();
        assert!(json.contains("\"type\":\"offer\""));

        // answer
        let answer = SignalingMessage::Answer {
            from_peer_id: "peer2".into(),
            to_peer_id: "peer1".into(),
            sdp: "v=0...ans".into(),
        };
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("\"type\":\"answer\""));

        // ice-candidate
        let candidate = SignalingMessage::IceCandidate {
            from_peer_id: "peer1".into(),
            to_peer_id: "peer2".into(),
            candidate: "candidate:1...".into(),
        };
        let json = serde_json::to_string(&candidate).unwrap();
        assert!(json.contains("\"type\":\"ice-candidate\""));

        // peer-left
        let left = SignalingMessage::PeerLeft {
            peer_id: "peer2".into(),
        };
        let json = serde_json::to_string(&left).unwrap();
        assert!(json.contains("\"type\":\"peer-left\""));

        // leave-room
        // knock / approval
        let requested = SignalingMessage::JoinRequested {
            peer_id: "peer2".into(),
            name: "Bob".into(),
        };
        let json = serde_json::to_string(&requested).unwrap();
        assert!(json.contains("\"type\":\"join-requested\""));

        let decision = SignalingMessage::JoinDecision {
            peer_id: "peer2".into(),
            accept: true,
        };
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("\"type\":\"join-decision\""));
        assert!(json.contains("\"accept\":true"));

        let leave = SignalingMessage::LeaveRoom {
            room_id: "room123".into(),
            peer_id: "peer1".into(),
        };
        let json = serde_json::to_string(&leave).unwrap();
        assert!(json.contains("\"type\":\"leave-room\""));
    }
}
