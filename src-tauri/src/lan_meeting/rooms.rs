use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, mpsc};

use super::protocol::{PeerInfo, SignalingMessage};

#[derive(Debug)]
pub struct RoomPeer {
    pub info: PeerInfo,
    pub tx: mpsc::UnboundedSender<SignalingMessage>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Room {
    pub id: String,
    pub host_peer_id: String,
    pub peers: HashMap<String, RoomPeer>,
    /// Guests who have knocked and are waiting on the host's verdict.
    pub pending: HashMap<String, RoomPeer>,
}

impl Room {
    pub fn new(id: String, host_peer_id: String) -> Self {
        Self {
            id,
            host_peer_id,
            peers: HashMap::new(),
            pending: HashMap::new(),
        }
    }
}

/// What should happen to a peer that just asked to join.
pub enum JoinOutcome {
    /// The first peer in the room is its host and walks straight in.
    Admitted {
        existing_peers: Vec<PeerInfo>,
        existing_senders: Vec<mpsc::UnboundedSender<SignalingMessage>>,
    },
    /// The host has been asked and the guest is waiting.
    AwaitingApproval {
        host_tx: mpsc::UnboundedSender<SignalingMessage>,
    },
}

#[derive(Clone, Default)]
pub struct RoomManager {
    rooms: Arc<RwLock<HashMap<String, Room>>>,
}

#[allow(dead_code)]
impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new room created by a host.
    pub async fn create_room(&self, room_id: &str, host_peer_id: &str) {
        let mut rooms = self.rooms.write().await;
        rooms.entry(room_id.to_string()).or_insert_with(|| {
            Room::new(room_id.to_string(), host_peer_id.to_string())
        });
    }

    /// Adds a peer to the specified room, or parks it until the host decides.
    ///
    /// The peer that opens a room is its host and is admitted immediately.
    /// Everyone after that waits in [`Room::pending`] until [`approve_peer`] or
    /// [`reject_peer`] is called for them.
    ///
    /// [`approve_peer`]: RoomManager::approve_peer
    /// [`reject_peer`]: RoomManager::reject_peer
    pub async fn join_peer(
        &self,
        room_id: &str,
        peer_id: String,
        name: String,
        tx: mpsc::UnboundedSender<SignalingMessage>,
    ) -> Result<JoinOutcome, String> {
        let mut rooms = self.rooms.write().await;
        let room = rooms
            .entry(room_id.to_string())
            .or_insert_with(|| Room::new(room_id.to_string(), peer_id.clone()));

        // Mesh architecture: limit to around 2-6 peers
        if room.peers.len() + room.pending.len() >= 6 {
            return Err("Room is full (maximum 6 participants)".to_string());
        }

        let is_host = room.peers.is_empty() || room.host_peer_id == peer_id;
        if is_host {
            room.host_peer_id = peer_id.clone();

            let existing_peers: Vec<PeerInfo> =
                room.peers.values().map(|p| p.info.clone()).collect();
            let existing_senders: Vec<mpsc::UnboundedSender<SignalingMessage>> =
                room.peers.values().map(|p| p.tx.clone()).collect();

            room.peers.insert(
                peer_id.clone(),
                RoomPeer {
                    info: PeerInfo {
                        peer_id,
                        name,
                        is_host,
                    },
                    tx,
                },
            );

            return Ok(JoinOutcome::Admitted {
                existing_peers,
                existing_senders,
            });
        }

        let host_tx = room
            .peers
            .get(&room.host_peer_id)
            .map(|host| host.tx.clone())
            .ok_or_else(|| "The host has left this room".to_string())?;

        room.pending.insert(
            peer_id.clone(),
            RoomPeer {
                info: PeerInfo {
                    peer_id,
                    name,
                    is_host: false,
                },
                tx,
            },
        );

        Ok(JoinOutcome::AwaitingApproval { host_tx })
    }

    /// Lets a waiting guest in, as the host asked.
    ///
    /// Returns the guest's own sender plus the peers already in the room, so the
    /// caller can send the welcome and announce the arrival.
    #[allow(clippy::type_complexity)]
    pub async fn approve_peer(
        &self,
        room_id: &str,
        peer_id: &str,
    ) -> Option<(
        PeerInfo,
        mpsc::UnboundedSender<SignalingMessage>,
        Vec<PeerInfo>,
        Vec<mpsc::UnboundedSender<SignalingMessage>>,
    )> {
        let mut rooms = self.rooms.write().await;
        let room = rooms.get_mut(room_id)?;
        let peer = room.pending.remove(peer_id)?;

        let existing_peers: Vec<PeerInfo> = room.peers.values().map(|p| p.info.clone()).collect();
        let existing_senders: Vec<mpsc::UnboundedSender<SignalingMessage>> =
            room.peers.values().map(|p| p.tx.clone()).collect();

        let info = peer.info.clone();
        let tx = peer.tx.clone();
        room.peers.insert(peer_id.to_string(), peer);

        Some((info, tx, existing_peers, existing_senders))
    }

    /// Turns a waiting guest away. Returns its sender so it can be told.
    pub async fn reject_peer(
        &self,
        room_id: &str,
        peer_id: &str,
    ) -> Option<mpsc::UnboundedSender<SignalingMessage>> {
        let mut rooms = self.rooms.write().await;
        let room = rooms.get_mut(room_id)?;
        room.pending.remove(peer_id).map(|peer| peer.tx)
    }

    /// Routes a direct signaling message (offer, answer, ice-candidate) to the target peer.
    pub async fn route_to_peer(
        &self,
        room_id: &str,
        to_peer_id: &str,
        msg: SignalingMessage,
    ) -> bool {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            if let Some(peer) = room.peers.get(to_peer_id) {
                let _ = peer.tx.send(msg);
                return true;
            }
        }
        false
    }

    /// Broadcasts a message to all peers in the room except `except_peer_id`.
    pub async fn broadcast_except(
        &self,
        room_id: &str,
        except_peer_id: &str,
        msg: SignalingMessage,
    ) {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            for (pid, peer) in &room.peers {
                if pid != except_peer_id {
                    let _ = peer.tx.send(msg.clone());
                }
            }
        }
    }

    /// Removes a peer from the room. Returns the remaining peer senders to notify.
    /// If the room becomes empty, the room is dropped from memory.
    pub async fn leave_peer(
        &self,
        room_id: &str,
        peer_id: &str,
    ) -> Vec<mpsc::UnboundedSender<SignalingMessage>> {
        let mut rooms = self.rooms.write().await;
        let mut remaining_senders = Vec::new();

        if let Some(room) = rooms.get_mut(room_id) {
            room.peers.remove(peer_id);
            room.pending.remove(peer_id);
            if room.peers.is_empty() {
                rooms.remove(room_id);
            } else {
                for peer in room.peers.values() {
                    remaining_senders.push(peer.tx.clone());
                }
            }
        }

        remaining_senders
    }

    /// Reports whether `peer_id` hosts the room, used to gate join decisions.
    pub async fn is_host(&self, room_id: &str, peer_id: &str) -> bool {
        let rooms = self.rooms.read().await;
        rooms
            .get(room_id)
            .is_some_and(|room| room.host_peer_id == peer_id)
    }

    /// Returns the current list of participants in a room.
    pub async fn list_peers(&self, room_id: &str) -> Vec<PeerInfo> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            room.peers.values().map(|p| p.info.clone()).collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted(
        outcome: JoinOutcome,
    ) -> (Vec<PeerInfo>, Vec<mpsc::UnboundedSender<SignalingMessage>>) {
        match outcome {
            JoinOutcome::Admitted {
                existing_peers,
                existing_senders,
            } => (existing_peers, existing_senders),
            JoinOutcome::AwaitingApproval { .. } => panic!("expected an immediate admission"),
        }
    }

    #[tokio::test]
    async fn test_room_lifecycle() {
        let mgr = RoomManager::new();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, _rx2) = mpsc::unbounded_channel();

        // Peer 1 opens the room and hosts it
        let (existing, senders) = admitted(
            mgr.join_peer("test-room", "peer1".into(), "Alice".into(), tx1)
                .await
                .unwrap(),
        );
        assert!(existing.is_empty());
        assert!(senders.is_empty());

        let peers = mgr.list_peers("test-room").await;
        assert_eq!(peers.len(), 1);
        assert!(peers[0].is_host);

        // Peer 2 knocks and is approved
        let outcome = mgr
            .join_peer("test-room", "peer2".into(), "Bob".into(), tx2)
            .await
            .unwrap();
        assert!(matches!(outcome, JoinOutcome::AwaitingApproval { .. }));

        let (info, _tx, existing2, senders2) =
            mgr.approve_peer("test-room", "peer2").await.unwrap();
        assert_eq!(info.name, "Bob");
        assert!(!info.is_host);
        assert_eq!(existing2.len(), 1);
        assert_eq!(existing2[0].peer_id, "peer1");
        assert_eq!(senders2.len(), 1);

        // Direct routing test
        let msg = SignalingMessage::Offer {
            from_peer_id: "peer2".into(),
            to_peer_id: "peer1".into(),
            sdp: "test-sdp".into(),
        };
        let routed = mgr.route_to_peer("test-room", "peer1", msg).await;
        assert!(routed);

        let received = rx1.recv().await.unwrap();
        match received {
            SignalingMessage::Offer { from_peer_id, .. } => {
                assert_eq!(from_peer_id, "peer2");
            }
            _ => panic!("Unexpected message"),
        }

        // Leave peer 2
        let remaining = mgr.leave_peer("test-room", "peer2").await;
        assert_eq!(remaining.len(), 1);

        // Leave peer 1 (empty room cleaned up)
        let remaining_after = mgr.leave_peer("test-room", "peer1").await;
        assert!(remaining_after.is_empty());
        assert!(mgr.list_peers("test-room").await.is_empty());
    }

    #[tokio::test]
    async fn a_guest_stays_out_until_the_host_approves() {
        let mgr = RoomManager::new();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, _guest_rx) = mpsc::unbounded_channel();

        admitted(
            mgr.join_peer("room", "host".into(), "Alice".into(), host_tx)
                .await
                .unwrap(),
        );

        let outcome = mgr
            .join_peer("room", "guest".into(), "Bob".into(), guest_tx)
            .await
            .unwrap();

        // The knock reaches the host, and the guest is not a participant yet.
        match outcome {
            JoinOutcome::AwaitingApproval { host_tx } => {
                host_tx
                    .send(SignalingMessage::JoinRequested {
                        peer_id: "guest".into(),
                        name: "Bob".into(),
                    })
                    .unwrap();
            }
            JoinOutcome::Admitted { .. } => panic!("a guest must not walk straight in"),
        }
        assert!(matches!(
            host_rx.recv().await.unwrap(),
            SignalingMessage::JoinRequested { .. }
        ));
        assert_eq!(mgr.list_peers("room").await.len(), 1);

        mgr.approve_peer("room", "guest").await.unwrap();
        assert_eq!(mgr.list_peers("room").await.len(), 2);
    }

    #[tokio::test]
    async fn a_rejected_guest_is_dropped_rather_than_admitted() {
        let mgr = RoomManager::new();
        let (host_tx, _host_rx) = mpsc::unbounded_channel();
        let (guest_tx, _guest_rx) = mpsc::unbounded_channel();

        admitted(
            mgr.join_peer("room", "host".into(), "Alice".into(), host_tx)
                .await
                .unwrap(),
        );
        mgr.join_peer("room", "guest".into(), "Bob".into(), guest_tx)
            .await
            .unwrap();

        assert!(mgr.reject_peer("room", "guest").await.is_some());
        assert_eq!(mgr.list_peers("room").await.len(), 1);
        // A second verdict on the same guest has nothing left to act on.
        assert!(mgr.approve_peer("room", "guest").await.is_none());
    }
}
