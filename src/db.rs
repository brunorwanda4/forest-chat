//! Turso persistence.
//!
//! Live traffic never waits on disk: writes go through an unbounded channel to a single
//! background task that drains it in batches and commits each batch in one transaction.
//! Reads (history, room list) use short-lived connections of their own.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use turso::{Builder, Connection, Database, Value};

use crate::protocol::{MemberInfo, RequestInfo};

/// Max writes committed in a single transaction.
const BATCH_SIZE: usize = 512;

/// Messages returned when a chat is opened.
pub const HISTORY_LIMIT: i64 = 100;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS users (
        name          TEXT PRIMARY KEY,
        password_hash TEXT NOT NULL,
        token         TEXT NOT NULL,
        created_ts    INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS rooms (
        name       TEXT PRIMARY KEY,
        creator    TEXT NOT NULL DEFAULT 'system',
        created_ts INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS room_members (
        room      TEXT NOT NULL,
        username  TEXT NOT NULL,
        role      TEXT NOT NULL,
        joined_ts INTEGER NOT NULL,
        PRIMARY KEY (room, username)
    );
    CREATE TABLE IF NOT EXISTS room_requests (
        room       TEXT NOT NULL,
        username   TEXT NOT NULL,
        created_ts INTEGER NOT NULL,
        PRIMARY KEY (room, username)
    );
    CREATE TABLE IF NOT EXISTS messages (
        id     INTEGER PRIMARY KEY,
        kind   TEXT NOT NULL,
        target TEXT NOT NULL,
        sender TEXT NOT NULL,
        body   TEXT NOT NULL,
        ts     INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_messages_chat ON messages (kind, target, id);
    CREATE TABLE IF NOT EXISTS attachments (
        id    TEXT PRIMARY KEY,
        name  TEXT NOT NULL,
        mime  TEXT NOT NULL,
        size  INTEGER NOT NULL,
        owner TEXT NOT NULL,
        ts    INTEGER NOT NULL
    );
";

#[derive(Debug, Clone, Serialize)]
pub struct StoredMessage {
    pub from: String,
    pub text: String,
    pub ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<Attachment>,
}

enum Write {
    Message {
        kind: &'static str,
        target: String,
        sender: String,
        body: String,
        ts: i64,
        attachment: Option<String>,
    },
}

#[derive(Clone)]
pub struct Store {
    db: Database,
    writes: mpsc::UnboundedSender<Write>,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b"$");
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_salt(name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.update(nanos.to_string().as_bytes());
    hasher.update(b"salt_seed");
    hex::encode(&hasher.finalize()[..8])
}

pub fn generate_token(name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.update(nanos.to_string().as_bytes());
    hasher.update(b"auth_token_key");
    hex::encode(hasher.finalize())
}

/// Conversation key for a direct chat; same for both participants.
pub fn dm_key(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}|{b}")
    } else {
        format!("{b}|{a}")
    }
}

impl Store {
    /// Opens (or creates) the database and starts the background writer.
    ///
    /// Must be called from within the Actix runtime.
    pub async fn open(path: &str) -> turso::Result<Self> {
        let db = Builder::new_local(path).build().await?;

        let conn = db.connect()?;
        conn.execute_batch(SCHEMA).await?;
        let _ = conn
            .execute(
                "ALTER TABLE rooms ADD COLUMN creator TEXT NOT NULL DEFAULT 'system'",
                (),
            )
            .await;
        // Older databases predate attachments; the error means the column is already there.
        let _ = conn
            .execute("ALTER TABLE messages ADD COLUMN attachment TEXT", ())
            .await;
        conn.execute(
            "INSERT OR IGNORE INTO rooms (name, creator, created_ts) VALUES (?1, ?2, ?3)",
            ("general", "system", now_ms()),
        )
        .await?;

        let (writes, rx) = mpsc::unbounded_channel();
        actix_web::rt::spawn(writer_loop(db.connect()?, rx));

        Ok(Self { db, writes })
    }

    pub async fn register_user(&self, name: &str, password: &str) -> Result<String, String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        let mut rows = conn
            .query("SELECT name FROM users WHERE name = ?1", (name,))
            .await
            .map_err(|e| e.to_string())?;
        if rows.next().await.map_err(|e| e.to_string())?.is_some() {
            return Err("Username is already taken.".into());
        }

        let salt = generate_salt(name);
        let hash = hash_password(password, &salt);
        let password_hash = format!("{salt}${hash}");
        let token = generate_token(name);
        let ts = now_ms();

        conn.execute(
            "INSERT INTO users (name, password_hash, token, created_ts) VALUES (?1, ?2, ?3, ?4)",
            (name, password_hash.as_str(), token.as_str(), ts),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(token)
    }

    pub async fn authenticate_user(&self, name: &str, password: &str) -> Result<String, String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        let mut rows = conn
            .query("SELECT password_hash FROM users WHERE name = ?1", (name,))
            .await
            .map_err(|e| e.to_string())?;
        let Some(row) = rows.next().await.map_err(|e| e.to_string())? else {
            return Err("Account not found. Please create an account.".into());
        };
        let stored_hash = text(row.get_value(0).map_err(|e| e.to_string())?);
        let parts: Vec<&str> = stored_hash.split('$').collect();
        if parts.len() != 2 {
            return Err("Corrupted account data.".into());
        }
        let salt = parts[0];
        let expected_hash = parts[1];
        let computed_hash = hash_password(password, salt);
        if computed_hash != expected_hash {
            return Err("Incorrect password.".into());
        }

        let token = generate_token(name);
        conn.execute(
            "UPDATE users SET token = ?1 WHERE name = ?2",
            (token.as_str(), name),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(token)
    }

    pub async fn verify_token(&self, name: &str, token: &str) -> bool {
        let Ok(conn) = self.db.connect() else {
            return false;
        };
        let Ok(mut rows) = conn
            .query("SELECT token FROM users WHERE name = ?1", (name,))
            .await
        else {
            return false;
        };
        match rows.next().await {
            Ok(Some(row)) => {
                if let Ok(val) = row.get_value(0) {
                    text(val) == token
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub async fn create_room(&self, room: &str, creator: &str) -> Result<(), String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        let mut rows = conn
            .query("SELECT name FROM rooms WHERE name = ?1", (room,))
            .await
            .map_err(|e| e.to_string())?;
        if rows.next().await.map_err(|e| e.to_string())?.is_some() {
            return Err("Group already exists.".into());
        }

        let ts = now_ms();
        conn.execute(
            "INSERT INTO rooms (name, creator, created_ts) VALUES (?1, ?2, ?3)",
            (room, creator, ts),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO room_members (room, username, role, joined_ts) VALUES (?1, ?2, 'creator', ?3)",
            (room, creator, ts),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn is_room_member(&self, room: &str, username: &str) -> bool {
        if room == "general" {
            return true;
        }
        let Ok(conn) = self.db.connect() else {
            return false;
        };
        let Ok(mut rows) = conn
            .query(
                "SELECT role FROM room_members WHERE room = ?1 AND username = ?2",
                (room, username),
            )
            .await
        else {
            return false;
        };
        match rows.next().await {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    pub async fn is_room_admin(&self, room: &str, username: &str) -> bool {
        let Ok(conn) = self.db.connect() else {
            return false;
        };
        let Ok(mut rows) = conn
            .query(
                "SELECT role FROM room_members WHERE room = ?1 AND username = ?2",
                (room, username),
            )
            .await
        else {
            return false;
        };
        match rows.next().await {
            Ok(Some(row)) => {
                if let Ok(val) = row.get_value(0) {
                    let r = text(val);
                    r == "creator" || r == "admin"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub async fn user_room_statuses(
        &self,
        username: &str,
    ) -> turso::Result<(Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query("SELECT name FROM rooms ORDER BY name", ())
            .await?;
        let mut all_rooms = Vec::new();
        while let Some(row) = rows.next().await? {
            all_rooms.push(text(row.get_value(0)?));
        }

        let mut joined_rooms = Vec::new();
        let mut admin_rooms = Vec::new();
        joined_rooms.push("general".to_owned());

        let mut member_rows = conn
            .query(
                "SELECT room, role FROM room_members WHERE username = ?1",
                (username,),
            )
            .await?;
        while let Some(row) = member_rows.next().await? {
            let r = text(row.get_value(0)?);
            let role = text(row.get_value(1)?);
            if !joined_rooms.contains(&r) {
                joined_rooms.push(r.clone());
            }
            if role == "creator" || role == "admin" {
                admin_rooms.push(r);
            }
        }

        let mut pending_rooms = Vec::new();
        let mut req_rows = conn
            .query(
                "SELECT room FROM room_requests WHERE username = ?1",
                (username,),
            )
            .await?;
        while let Some(row) = req_rows.next().await? {
            pending_rooms.push(text(row.get_value(0)?));
        }

        Ok((all_rooms, joined_rooms, pending_rooms, admin_rooms))
    }

    pub async fn request_join(&self, room: &str, username: &str) -> Result<(), String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        let ts = now_ms();
        conn.execute(
            "INSERT OR IGNORE INTO room_requests (room, username, created_ts) VALUES (?1, ?2, ?3)",
            (room, username, ts),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn approve_join(&self, room: &str, username: &str) -> Result<(), String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        let ts = now_ms();
        conn.execute(
            "DELETE FROM room_requests WHERE room = ?1 AND username = ?2",
            (room, username),
        )
        .await
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO room_members (room, username, role, joined_ts) VALUES (?1, ?2, 'member', ?3)",
            (room, username, ts),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn reject_join(&self, room: &str, username: &str) -> Result<(), String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM room_requests WHERE room = ?1 AND username = ?2",
            (room, username),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn promote_admin(&self, room: &str, username: &str) -> Result<(), String> {
        let conn = self.db.connect().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE room_members SET role = 'admin' WHERE room = ?1 AND username = ?2",
            (room, username),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_room_details(
        &self,
        room: &str,
    ) -> turso::Result<(Vec<MemberInfo>, Vec<RequestInfo>)> {
        let conn = self.db.connect()?;
        let mut members = Vec::new();
        let mut rows = conn
            .query(
                "SELECT username, role, joined_ts FROM room_members WHERE room = ?1 ORDER BY joined_ts ASC",
                (room,),
            )
            .await?;
        while let Some(row) = rows.next().await? {
            members.push(MemberInfo {
                username: text(row.get_value(0)?),
                role: text(row.get_value(1)?),
                joined_ts: integer(row.get_value(2)?),
            });
        }

        let mut requests = Vec::new();
        let mut req_rows = conn
            .query(
                "SELECT username, created_ts FROM room_requests WHERE room = ?1 ORDER BY created_ts ASC",
                (room,),
            )
            .await?;
        while let Some(row) = req_rows.next().await? {
            requests.push(RequestInfo {
                username: text(row.get_value(0)?),
                created_ts: integer(row.get_value(1)?),
            });
        }

        Ok((members, requests))
    }

    pub async fn get_room_admins(&self, room: &str) -> turso::Result<Vec<String>> {
        let conn = self.db.connect()?;
        let mut admins = Vec::new();
        let mut rows = conn
            .query(
                "SELECT username FROM room_members WHERE room = ?1 AND (role = 'creator' OR role = 'admin')",
                (room,),
            )
            .await?;
        while let Some(row) = rows.next().await? {
            admins.push(text(row.get_value(0)?));
        }
        Ok(admins)
    }

    pub async fn rooms(&self) -> turso::Result<Vec<String>> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query("SELECT name FROM rooms ORDER BY name", ())
            .await?;

        let mut rooms = Vec::new();
        while let Some(row) = rows.next().await? {
            rooms.push(text(row.get_value(0)?));
        }
        Ok(rooms)
    }

    /// Latest messages of a chat, oldest first.
    pub async fn history(&self, kind: &str, target: &str) -> turso::Result<Vec<StoredMessage>> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query(
                "SELECT sender, body, ts FROM messages
                 WHERE kind = ?1 AND target = ?2
                 ORDER BY id DESC LIMIT ?3",
                (kind, target, HISTORY_LIMIT),
            )
            .await?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next().await? {
            messages.push(StoredMessage {
                from: text(row.get_value(0)?),
                text: text(row.get_value(1)?),
                ts: integer(row.get_value(2)?),
            });
        }
        messages.reverse();
        Ok(messages)
    }

    pub fn save_message(
        &self,
        kind: &'static str,
        target: String,
        sender: &str,
        body: &str,
        ts: i64,
    ) {
        self.enqueue(Write::Message {
            kind,
            target,
            sender: sender.to_owned(),
            body: body.to_owned(),
            ts,
        });
    }

    fn enqueue(&self, write: Write) {
        if self.writes.send(write).is_err() {
            log::error!("database writer stopped; dropping write");
        }
    }
}

async fn writer_loop(conn: Connection, mut rx: mpsc::UnboundedReceiver<Write>) {
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    while rx.recv_many(&mut batch, BATCH_SIZE).await > 0 {
        if let Err(err) = write_batch(&conn, &batch).await {
            log::error!("failed to persist {} writes: {err}", batch.len());
            let _ = conn.execute("ROLLBACK", ()).await;
        }
        batch.clear();
    }
}

async fn write_batch(conn: &Connection, batch: &[Write]) -> turso::Result<()> {
    let started = std::time::Instant::now();
    conn.execute("BEGIN", ()).await?;

    for write in batch {
        match write {
            Write::Message {
                kind,
                target,
                sender,
                body,
                ts,
            } => {
                conn.prepare_cached(
                    "INSERT INTO messages (kind, target, sender, body, ts) VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .await?
                .execute((*kind, target.as_str(), sender.as_str(), body.as_str(), *ts))
                .await?;
            }
        }
    }

    conn.execute("COMMIT", ()).await?;
    log::debug!(
        "persisted {} writes in {:?}",
        batch.len(),
        started.elapsed()
    );
    Ok(())
}

fn text(value: Value) -> String {
    match value {
        Value::Text(s) => s,
        _ => String::new(),
    }
}

fn integer(value: Value) -> i64 {
    match value {
        Value::Integer(n) => n,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let salt = "mysalt123";
        let h1 = hash_password("secretpass", salt);
        let h2 = hash_password("secretpass", salt);
        let h3 = hash_password("different", salt);
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[actix_web::test]
    async fn test_auth_and_groups() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test-forest.db");
        let store = Store::open(db_path.to_str().unwrap()).await.unwrap();

        // 1. Register Alice
        let alice_token = store.register_user("alice", "password123").await.unwrap();
        assert!(!alice_token.is_empty());
        assert!(store.verify_token("alice", &alice_token).await);
        assert!(!store.verify_token("alice", "wrong_token").await);

        // Duplicate registration should fail
        assert!(store.register_user("alice", "password123").await.is_err());

        // 2. Login Alice
        let login_token = store.authenticate_user("alice", "password123").await.unwrap();
        assert!(store.verify_token("alice", &login_token).await);
        assert!(store.authenticate_user("alice", "wrong_pw").await.is_err());
        assert!(store.authenticate_user("nonexistent", "pw").await.is_err());

        // 3. Register Bob & Charlie
        let _bob_token = store.register_user("bob", "password123").await.unwrap();
        let _charlie_token = store.register_user("charlie", "password123").await.unwrap();

        // 4. Default room 'general' is joined by all
        let (all, joined, _, _) = store.user_room_statuses("alice").await.unwrap();
        assert!(all.contains(&"general".to_string()));
        assert!(joined.contains(&"general".to_string()));
        assert!(store.is_room_member("general", "alice").await);
        assert!(store.is_room_member("general", "bob").await);

        // 5. Alice creates a group 'rust-dev'
        store.create_room("rust-dev", "alice").await.unwrap();
        assert!(store.create_room("rust-dev", "bob").await.is_err()); // duplicate group creation should fail
        assert!(store.is_room_member("rust-dev", "alice").await);
        assert!(store.is_room_admin("rust-dev", "alice").await);
        assert!(!store.is_room_member("rust-dev", "bob").await);
        assert!(!store.is_room_admin("rust-dev", "bob").await);

        // 6. Bob and Charlie can see 'rust-dev' in all_rooms, but not joined
        let (all, joined, _, _) = store.user_room_statuses("bob").await.unwrap();
        assert!(all.contains(&"rust-dev".to_string()));
        assert!(!joined.contains(&"rust-dev".to_string()));

        // 7. Bob requests to join 'rust-dev'
        store.request_join("rust-dev", "bob").await.unwrap();
        let (_, _, pending, _) = store.user_room_statuses("bob").await.unwrap();
        assert!(pending.contains(&"rust-dev".to_string()));

        // Alice inspects requests
        let (members, reqs) = store.get_room_details("rust-dev").await.unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].username, "alice");
        assert_eq!(members[0].role, "creator");
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].username, "bob");

        // 8. Alice approves Bob
        store.approve_join("rust-dev", "bob").await.unwrap();
        assert!(store.is_room_member("rust-dev", "bob").await);
        assert!(!store.is_room_admin("rust-dev", "bob").await);

        let (_, joined_bob, pending_bob, _) = store.user_room_statuses("bob").await.unwrap();
        assert!(joined_bob.contains(&"rust-dev".to_string()));
        assert!(!pending_bob.contains(&"rust-dev".to_string()));

        // 9. Charlie requests to join and Alice rejects
        store.request_join("rust-dev", "charlie").await.unwrap();
        store.reject_join("rust-dev", "charlie").await.unwrap();
        assert!(!store.is_room_member("rust-dev", "charlie").await);
        let (_, _, pending_charlie, _) = store.user_room_statuses("charlie").await.unwrap();
        assert!(!pending_charlie.contains(&"rust-dev".to_string()));

        // 10. Alice promotes Bob to admin
        store.promote_admin("rust-dev", "bob").await.unwrap();
        assert!(store.is_room_admin("rust-dev", "bob").await);
        let (_, _, _, admin_bob) = store.user_room_statuses("bob").await.unwrap();
        assert!(admin_bob.contains(&"rust-dev".to_string()));

        // 11. Now Bob as admin can approve Charlie's new request
        store.request_join("rust-dev", "charlie").await.unwrap();
        store.approve_join("rust-dev", "charlie").await.unwrap();
        assert!(store.is_room_member("rust-dev", "charlie").await);
    }
}

