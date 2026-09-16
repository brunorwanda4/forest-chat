import type { ServerMsg } from "./types";
import { state, send, chatState } from "./state";
import { $, toast } from "./utils/dom";
import { sameChat } from "./utils/format";
import { showApp, showLogin } from "./views/auth";
import { renderRooms, renderSidebar } from "./views/sidebar";
import {
  render,
  renderHeader,
  renderMessages,
  appendMessage,
  openChat,
  requestHistory,
  setTyping,
  clearTyping,
} from "./views/chat";
import { renderManageGroupModal } from "./components/modals";

export function setStatus(text: string, cls: string): void {
  const el = $("status");
  if (!el) return;
  el.textContent = text;
  el.className = `badge badge-sm ${cls}`;
}

const handlers: Record<string, (msg: any) => void> = {
  welcome(msg) {
    state.me = msg.me;
    state.users = msg.users;
    state.rooms = msg.rooms;
    state.joined = new Set(msg.joined_rooms);
    state.pending = new Set(msg.pending_rooms);
    state.adminRooms = new Set(msg.admin_rooms);
    setStatus("online", "badge-success");

    for (const chat of state.chats.values()) {
      chat.loaded = false;
    }
    state.typing.clear();
    for (const room of state.joined) {
      send({ type: "join", room });
    }

    showApp();
    if (state.active && state.joined.has(state.active.id)) {
      openChat(state.active);
    } else if (state.joined.has("general")) {
      openChat({ kind: "room", id: "general" });
    }
    render();
  },

  presence(msg) {
    state.users = msg.users;
    render();
  },

  rooms(msg) {
    state.rooms = msg.rooms;
    state.joined = new Set(msg.joined_rooms);
    state.pending = new Set(msg.pending_rooms);
    state.adminRooms = new Set(msg.admin_rooms);
    renderRooms();
    renderHeader();
  },

  joined(msg) {
    const isNew = !state.joined.has(msg.room);
    state.joined.add(msg.room);
    state.pending.delete(msg.room);
    if (isNew) {
      openChat({ kind: "room", id: msg.room });
    } else if (sameChat(state.active, { kind: "room", id: msg.room })) {
      requestHistory(state.active!);
    }
    renderRooms();
    renderHeader();
  },

  room_details(msg) {
    state.activeRoomDetails = msg;
    renderManageGroupModal();
    renderHeader();
  },

  join_requested(msg) {
    toast(`@${msg.user} requested to join #${msg.room}`, "info");
    if (
      state.active?.kind === "room" &&
      state.active.id === msg.room &&
      state.adminRooms.has(msg.room)
    ) {
      send({ type: "get_room_details", room: msg.room });
    }
  },

  join_approved(msg) {
    if (msg.user === state.me) {
      toast(`Your request to join #${msg.room} was approved! 🎉`, "success");
      state.joined.add(msg.room);
      state.pending.delete(msg.room);
      send({ type: "join", room: msg.room });
      openChat({ kind: "room", id: msg.room });
      renderRooms();
    }
  },

  join_rejected(msg) {
    if (msg.user === state.me) {
      toast(`Your request to join #${msg.room} was declined.`, "warning");
      state.pending.delete(msg.room);
      renderRooms();
    }
  },

  admin_promoted(msg) {
    if (msg.user === state.me) {
      toast(`You were promoted to Admin in #${msg.room}! 🛡️`, "success");
      state.adminRooms.add(msg.room);
      renderRooms();
      renderHeader();
    }
    if (state.active?.kind === "room" && state.active.id === msg.room) {
      send({ type: "get_room_details", room: msg.room });
    }
  },

  history(msg) {
    const chat = chatState(msg.chat);
    const seen = new Set(
      msg.messages.map((m: any) => `${m.ts}|${m.from}|${m.text}`)
    );
    const live = chat.messages.filter(
      (m) => !seen.has(`${m.ts}|${m.from}|${m.text}`)
    );
    chat.messages = [...msg.messages, ...live];
    chat.loaded = true;
    if (sameChat(state.active, msg.chat)) {
      renderMessages();
    }
  },

  message(msg) {
    const chat = chatState(msg.chat);
    chat.messages.push({
      from: msg.from,
      text: msg.text,
      ts: msg.ts,
      attachment: msg.attachment,
    });
    if (msg.chat.kind === "dm") state.dmPeers.add(msg.chat.id);
    clearTyping(msg.chat, msg.from);

    if (sameChat(state.active, msg.chat)) {
      appendMessage(chat.messages.at(-1)!, chat.messages.at(-2));
    } else if (msg.from !== state.me) {
      chat.unread += 1;
    }
    renderSidebar();
  },

  typing(msg) {
    if (msg.active) setTyping(msg.chat, msg.from);
    else clearTyping(msg.chat, msg.from);
  },

  error(msg) {
    toast(msg.message);
  },
};

export function connect(name: string, token: string): void {
  if (state.retry) clearTimeout(state.retry);
  state.me = name;
  state.token = token;
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const ws = new WebSocket(
    `${proto}://${location.host}/ws?name=${encodeURIComponent(name)}&token=${encodeURIComponent(token)}`
  );
  state.ws = ws;
  let rejected = false;
  setStatus("connecting…", "badge-ghost");

  ws.onmessage = (event: MessageEvent) => {
    let msg: ServerMsg;
    try {
      msg = JSON.parse(event.data);
    } catch {
      return;
    }

    if (msg.type === "error" && !state.me) {
      rejected = true;
      showLogin((msg as any).message);
      return;
    }

    const handler = handlers[msg.type];
    if (handler) {
      handler(msg);
    }
  };

  ws.onclose = () => {
    if (state.ws !== ws) return;
    if (rejected || !state.me) return;
    setStatus("offline", "badge-error");
    state.retry = setTimeout(() => {
      if (state.me && state.token) {
        connect(state.me, state.token);
      }
    }, 2000);
  };
}
