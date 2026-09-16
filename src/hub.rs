//! In-memory routing of live traffic: who is online and who is in which room.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use bytestring::ByteString;
use tokio::sync::mpsc::UnboundedSender;

/// Pre-serialized frames; cloning a `ByteString` only bumps a refcount.
pub type Outbox = UnboundedSender<ByteString>;

#[derive(Default)]
struct Inner {
    users: HashMap<String, Outbox>,
    /// Room name -> online members.
    rooms: HashMap<String, HashSet<String>>,
}

#[derive(Clone, Default)]
pub struct Hub {
    inner: Arc<Mutex<Inner>>,
}

impl Hub {
    pub fn with_rooms(rooms: impl IntoIterator<Item = String>) -> Self {
        let hub = Self::default();
        hub.lock()
            .rooms
            .extend(rooms.into_iter().map(|room| (room, HashSet::new())));
        hub
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Registers a user. Returns `false` if the name is already online.
    pub fn connect(&self, name: &str, outbox: Outbox) -> bool {
        let mut inner = self.lock();
        if inner.users.contains_key(name) {
            return false;
        }
        inner.users.insert(name.to_owned(), outbox);
        true
    }

    pub fn disconnect(&self, name: &str) {
        let mut inner = self.lock();
        inner.users.remove(name);
        for members in inner.rooms.values_mut() {
            members.remove(name);
        }
    }

    pub fn online(&self) -> Vec<String> {
        let mut users: Vec<_> = self.lock().users.keys().cloned().collect();
        users.sort_unstable();
        users
    }

    pub fn rooms(&self) -> Vec<String> {
        let mut rooms: Vec<_> = self.lock().rooms.keys().cloned().collect();
        rooms.sort_unstable();
        rooms
    }

    /// Adds `name` to `room`, creating the room if needed. Returns `true` if it was created.
    pub fn join(&self, room: &str, name: &str) -> bool {
        let mut inner = self.lock();
        match inner.rooms.get_mut(room) {
            Some(members) => {
                members.insert(name.to_owned());
                false
            }
            None => {
                inner
                    .rooms
                    .insert(room.to_owned(), HashSet::from([name.to_owned()]));
                true
            }
        }
    }

    pub fn leave(&self, room: &str, name: &str) {
        if let Some(members) = self.lock().rooms.get_mut(room) {
            members.remove(name);
        }
    }

    pub fn is_member(&self, room: &str, name: &str) -> bool {
        self.lock()
            .rooms
            .get(room)
            .is_some_and(|members| members.contains(name))
    }

    /// Returns `false` if the user is offline.
    pub fn send_to_user(&self, name: &str, frame: &ByteString) -> bool {
        match self.lock().users.get(name) {
            Some(outbox) => outbox.send(frame.clone()).is_ok(),
            None => false,
        }
    }

    pub fn send_to_room(&self, room: &str, frame: &ByteString, except: Option<&str>) {
        let inner = self.lock();
        let Some(members) = inner.rooms.get(room) else {
            return;
        };
        for member in members {
            if Some(member.as_str()) == except {
                continue;
            }
            if let Some(outbox) = inner.users.get(member) {
                let _ = outbox.send(frame.clone());
            }
        }
    }

    pub fn add_room(&self, room: &str) {
        let mut inner = self.lock();
        inner.rooms.entry(room.to_owned()).or_default();
    }

    pub fn send_to_users(&self, users: &[String], frame: &ByteString) {
        let inner = self.lock();
        for user in users {
            if let Some(outbox) = inner.users.get(user) {
                let _ = outbox.send(frame.clone());
            }
        }
    }

    pub fn broadcast(&self, frame: &ByteString) {
        for outbox in self.lock().users.values() {
            let _ = outbox.send(frame.clone());
        }
    }
}
