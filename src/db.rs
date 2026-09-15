//! Turso persistence.
//!
//! Live traffic never waits on disk: writes go through an unbounded channel to a single
//! background task that drains it in batches and commits each batch in one transaction.
//! Reads (history, room list) use short-lived connections of their own.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::mpsc;
use turso::{Builder, Connection, Database, Value};

/// Max writes committed in a single transaction.
const BATCH_SIZE: usize = 512;

/// Messages returned when a chat is opened.
pub const HISTORY_LIMIT: i64 = 100;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS rooms (
        name       TEXT PRIMARY KEY,
        created_ts INTEGER NOT NULL
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
";

#[derive(Debug, Clone, Serialize)]
pub struct StoredMessage {
    pub from: String,
    pub text: String,
    pub ts: i64,
}

enum Write {
    Room {
        name: String,
        ts: i64,
    },
    Message {
        kind: &'static str,
        target: String,
        sender: String,
        body: String,
        ts: i64,
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
        conn.execute(
            "INSERT OR IGNORE INTO rooms (name, created_ts) VALUES (?1, ?2)",
            ("general", now_ms()),
        )
        .await?;

        let (writes, rx) = mpsc::unbounded_channel();
        actix_web::rt::spawn(writer_loop(db.connect()?, rx));

        Ok(Self { db, writes })
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

    pub fn save_room(&self, name: &str) {
        self.enqueue(Write::Room {
            name: name.to_owned(),
            ts: now_ms(),
        });
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
            Write::Room { name, ts } => {
                conn.prepare_cached(
                    "INSERT OR IGNORE INTO rooms (name, created_ts) VALUES (?1, ?2)",
                )
                .await?
                .execute((name.as_str(), *ts))
                .await?;
            }
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
