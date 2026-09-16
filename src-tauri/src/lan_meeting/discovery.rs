//! Finds meeting rooms on the local network so nobody has to type an IP.
//!
//! A host answers UDP probes on [`DISCOVERY_PORT`] with a JSON description of
//! its room. Anyone looking for rooms broadcasts one probe and collects the
//! replies for [`DISCOVERY_WINDOW`]. Nothing is remembered between scans, so a
//! room disappears from the list as soon as its host stops answering.

use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

pub const DISCOVERY_PORT: u16 = 45477;
const PROBE: &[u8] = b"FOREST_CHAT_DISCOVER_V1";
const DISCOVERY_WINDOW: Duration = Duration::from_millis(700);
const MAX_DATAGRAM: usize = 2 * 1024;

/// What a host broadcasts about its room, and what a scan returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRoom {
    pub room_id: String,
    pub host_name: String,
    pub host_ip: String,
    pub port: u16,
    pub participant_count: usize,
}

/// The room a host is currently advertising, shared with the responder thread.
///
/// `None` means the host is not in a meeting, and probes go unanswered.
pub type Advertised = Arc<Mutex<Option<DiscoveredRoom>>>;

/// Answers discovery probes for as long as `advertised` holds a room.
///
/// Returns immediately if another process already owns the discovery port; the
/// meeting still works, callers just have to share the address by hand.
pub fn start_responder(advertised: Advertised, running: Arc<AtomicBool>) {
    if running.swap(true, Ordering::SeqCst) {
        return;
    }

    let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)) {
        Ok(socket) => socket,
        Err(e) => {
            log::warn!("LAN discovery unavailable, port {DISCOVERY_PORT} is busy: {e}");
            running.store(false, Ordering::SeqCst);
            return;
        }
    };

    std::thread::spawn(move || {
        log::info!("LAN discovery responder listening on {DISCOVERY_PORT}");
        let mut buf = [0u8; MAX_DATAGRAM];

        while running.load(Ordering::Relaxed) {
            let (len, from) = match socket.recv_from(&mut buf) {
                Ok(result) => result,
                Err(e) => {
                    log::debug!("Discovery socket error: {e}");
                    continue;
                }
            };

            if &buf[..len] != PROBE {
                continue;
            }

            let room = advertised.lock().ok().and_then(|room| room.clone());
            if let Some(room) = room {
                if let Ok(json) = serde_json::to_vec(&room) {
                    let _ = socket.send_to(&json, from);
                }
            }
        }

        log::info!("LAN discovery responder stopped");
    });
}

/// Broadcasts one probe and collects every room that answers in time.
pub fn scan() -> Result<Vec<DiscoveredRoom>, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|e| format!("Failed to open discovery socket: {e}"))?;
    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(120)))
        .map_err(|e| format!("Failed to set discovery timeout: {e}"))?;

    let broadcast: SocketAddr = (Ipv4Addr::BROADCAST, DISCOVERY_PORT).into();
    socket
        .send_to(PROBE, broadcast)
        .map_err(|e| format!("Failed to send discovery probe: {e}"))?;

    let mut rooms: Vec<DiscoveredRoom> = Vec::new();
    let mut buf = [0u8; MAX_DATAGRAM];
    let deadline = Instant::now() + DISCOVERY_WINDOW;

    while Instant::now() < deadline {
        let (len, from) = match socket.recv_from(&mut buf) {
            Ok(result) => result,
            Err(_) => continue, // Timed out waiting for this slice of the window.
        };

        let Ok(mut room) = serde_json::from_slice::<DiscoveredRoom>(&buf[..len]) else {
            continue;
        };

        // Trust the address the reply came from over whatever the host reported,
        // which may be a stale or badly detected interface address.
        room.host_ip = from.ip().to_string();

        if !rooms.iter().any(|known| known.room_id == room.room_id) {
            rooms.push(room);
        }
    }

    Ok(rooms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_survives_a_json_round_trip() {
        let room = DiscoveredRoom {
            room_id: "a1b2c3".into(),
            host_name: "Rwanda".into(),
            host_ip: "192.168.1.78".into(),
            port: 8080,
            participant_count: 2,
        };

        let json = serde_json::to_vec(&room).unwrap();
        let back: DiscoveredRoom = serde_json::from_slice(&json).unwrap();

        assert_eq!(back.room_id, "a1b2c3");
        assert_eq!(back.port, 8080);
        assert_eq!(back.participant_count, 2);
    }

    #[test]
    fn a_scan_finishes_and_never_lists_the_same_room_twice() {
        // Whoever is on this network when the suite runs, a scan must come back
        // within its window and must not repeat a room that answered twice.
        let rooms = scan().expect("a scan should not fail");

        let mut ids: Vec<&str> = rooms.iter().map(|room| room.room_id.as_str()).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();

        assert_eq!(ids.len(), total, "a room was listed more than once");
    }
}
