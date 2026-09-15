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
}

impl Room {
    pub fn new(id: String, host_peer_id: String) -> Self {
        Self {
            id,
            host_peer_id,
            peers: HashMap::new(),
        }
    }
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

    /// Adds a peer to the specified room.
    /// Returns a list of existing peers in the room (excluding the newly joined peer)
    /// and a list of senders for all existing peers so they can be notified.
    pub async fn join_peer(
        &self,
        room_id: &str,
        peer_id: String,
        name: String,
        tx: mpsc::UnboundedSender<SignalingMessage>,
    ) -> Result<(Vec<PeerInfo>, Vec<mpsc::UnboundedSender<SignalingMessage>>), String> {
        let mut rooms = self.rooms.write().await;
        let room = rooms.entry(room_id.to_string()).or_insert_with(|| {
            Room::new(room_id.to_string(), peer_id.clone())
        });

        // Mesh architecture: limit to around 2-6 peers
        if room.peers.len() >= 6 {
            return Err("Room is full (maximum 6 participants)".to_string());
        }

        let is_host = room.peers.is_empty() || room.host_peer_id == peer_id;
        if is_host {
            room.host_peer_id = peer_id.clone();
        }

        let existing_peers: Vec<PeerInfo> = room.peers.values().map(|p| p.info.clone()).collect();
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

        Ok((existing_peers, existing_senders))
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

    #[tokio::test]
    async fn test_room_lifecycle() {
        let mgr = RoomManager::new();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, _rx2) = mpsc::unbounded_channel();

        // Peer 1 joins (becomes host)
        let (existing, senders) = mgr
            .join_peer("test-room", "peer1".into(), "Alice".into(), tx1)
            .await
            .unwrap();
        assert!(existing.is_empty());
        assert!(senders.is_empty());

        let peers = mgr.list_peers("test-room").await;
        assert_eq!(peers.len(), 1);
        assert!(peers[0].is_host);

        // Peer 2 joins
        let (existing2, senders2) = mgr
            .join_peer("test-room", "peer2".into(), "Bob".into(), tx2)
            .await
            .unwrap();
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
}

