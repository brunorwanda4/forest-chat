# Forest Chat

Small real-time chat built with Actix Web + WebSockets, stored in [Turso](https://github.com/tursodatabase/turso).
UI uses daisyUI (Tailwind) with the `forest` theme and [DiceBear](https://www.dicebear.com/) avatars.

## Features

- Group chats: join existing groups or create one by typing a name
- Direct messages to a single person
- "is typing…" indicator in the chat and the sidebar
- Online/offline presence
- Message history saved to a local Turso database

## Run in a browser

```sh
cargo run -p forest-chat --release
```

Open <http://localhost:8081>. Open a second browser (or a private window) with another name to chat with yourself.

| Env var         | Default          |
| --------------- | ---------------- |
| `PORT`          | `8081`           |
| `DATABASE_PATH` | `forest-chat.db` |

## Run as a desktop app

Forest Chat includes a Tauri 2 desktop shell. On Windows, install the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) first, then run:

```sh
cargo run -p forest-chat-desktop -j 1
```

The desktop app starts the chat server on a private local port and keeps its database in the
operating system's application-data directory. To connect desktop clients to a shared deployed
Forest Chat server, set `FOREST_CHAT_SERVER_URL` before starting the app:

```powershell
$env:FOREST_CHAT_SERVER_URL = "https://chat.example.com"
cargo run -p forest-chat-desktop -j 1
```

To create a Windows installer, install the Tauri CLI and build the bundle:

```sh
cargo install tauri-cli --version "^2.0.0" --locked
cargo tauri build
```

If the Rust compiler runs out of memory, set `CARGO_BUILD_JOBS=1` and run the build again.

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

## LAN Real-Time Meeting & Screen Sharing (Rust WebRTC)

Forest Chat includes a pure Rust, real-time peer-to-peer LAN meeting and streaming feature designed for local networks and offline environments (such as homes, offices, schools, and conferences) without requiring internet or cloud services.

### Key Architecture & Tech Stack

- **WebRTC Mesh (`webrtc-rs`)**: Full peer-to-peer mesh architecture (supporting 2–6 participants) where all media connections and data channels are managed strictly within Rust. No JavaScript WebRTC APIs, React WebRTC wrappers, or third-party cloud signaling services are used.
- **Microphone Audio (`cpal`)**: Real-time microphone input capture with hardware downmixing, volume RMS speaking detection, and speaker playback of remote peer audio streams.
- **Screen Capture (`xcap`)**: High-performance primary monitor capture with hardware-friendly downsampling and low-latency JPEG compression (~15–20 fps).
- **In-Room Chat (`RTCDataChannel`)**: End-to-end peer-to-peer text chat and media control messages broadcast directly over WebRTC data channels.
- **Signaling Server (`actix-web` + `actix-ws`)**: In-memory room manager binding to `0.0.0.0` to coordinate offer/answer negotiations and ICE candidate exchanges across the local network.
- **Tauri UI (Tailwind CSS + DaisyUI)**: Intuitive meeting lobby, stage with hardware-accelerated `<canvas>` screen rendering, participant status list, and media controls (Mute, Screen Share, Leave).

### How Hosting Works

1. Open the Forest Chat desktop app and click the **"LAN Meeting"** tab in the sidebar.
2. Under the **"Host Room"** tab, verify your detected local LAN IP (e.g. `192.168.1.5`) and signaling port (default `8080`).
3. Click **"Create Room & Start Meeting"**.
4. The host application will display:
   - **Room Code** (e.g., `#A1B2C3`)
   - **Local LAN IP** (e.g., `192.168.1.5`)
   - **Join Address** (e.g., `ws://192.168.1.5:8080/ws/meeting/a1b2c3`)
5. Click **"Copy Join Address"** to copy the full connection link to your clipboard and share it with peers on the same network.

### How Joining Works

1. Launch Forest Chat on another computer connected to the same Wi-Fi or local area network.
2. Click the **"LAN Meeting"** tab and select the **"Join Room"** tab.
3. Enter:
   - Your Display Name
   - Host's LAN IP address (e.g. `192.168.1.5:8080` or the full `ws://...` join address)
   - The 6-character Room Code (e.g. `a1b2c3`)
4. Click **"Connect & Join Meeting"**.
5. Once connected, a direct WebRTC peer-to-peer mesh connection is established between all participants in the room.

### Network & Firewall Requirements

- **Same Subnet / Wi-Fi**: Both host and guests must be on the same local network subnet (or VPN LAN).
- **Windows Firewall**: When hosting for the first time, Windows Defender Firewall may display a prompt asking to allow network access. Click **"Allow access" on Private Networks** to permit incoming WebSocket signaling connections on port 8080.
- **Port Selection**: If port `8080` is already in use by another local service, you can specify any open port (e.g., `8085` or `9000`) in the Host form before creating the room.

## ⚠️ Not production-ready

There are no accounts or passwords. Anyone who picks a name that is currently offline can
read that name's direct-message history. Add real authentication before exposing this beyond
your own machine or LAN.

