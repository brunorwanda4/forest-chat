# Forest Chat

Small real-time chat built with Actix Web + WebSockets, stored in [Turso](https://github.com/tursodatabase/turso).
UI uses daisyUI (Tailwind) with the `forest` theme and [DiceBear](https://www.dicebear.com/) avatars.

## Features

- Group chats: join existing groups or create one by typing a name
- Direct messages to a single person
- "is typing…" indicator in the chat and the sidebar
- Online/offline presence
- Message history saved to a local Turso database

## Run

```sh
cargo run --release
```

Open <http://localhost:8081>. Open a second browser (or a private window) with another name to chat with yourself.

| Env var         | Default          |
| --------------- | ---------------- |
| `PORT`          | `8081`           |
| `DATABASE_PATH` | `forest-chat.db` |

Run from the project folder so `./static` is found.

## How it stays fast

- Live messages are routed in memory (`src/hub.rs`) and serialized once per send; fan-out
  clones a refcounted buffer, not the JSON.
- Database writes never block chatting. They go through a channel to one background writer
  (`src/db.rs`) that commits up to 512 writes per transaction.
- History (last 100 messages) is loaded only when a chat is opened, using an index on
  `(kind, target, id)`.

## Protocol

Client → server (JSON over `/ws?name=<name>`):

```json
{ "type": "join", "room": "general" }
{ "type": "leave", "room": "general" }
{ "type": "send", "chat": { "kind": "room", "id": "general" }, "text": "hi" }
{ "type": "send", "chat": { "kind": "dm", "id": "alice" }, "text": "hi" }
{ "type": "typing", "chat": { "kind": "dm", "id": "alice" }, "active": true }
{ "type": "history", "chat": { "kind": "room", "id": "general" } }
```

Server → client: `welcome`, `presence`, `rooms`, `joined`, `message`, `typing`, `history`, `error`.

## ⚠️ Not production-ready

There are no accounts or passwords. Anyone who picks a name that is currently offline can
read that name's direct-message history. Add real authentication before exposing this beyond
your own machine or LAN.
