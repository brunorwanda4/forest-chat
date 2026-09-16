import type { ActiveChat, ChatMessage } from "../types";
import { state, send, chatState } from "../state";
import { $, h, avatar, avatarUrl, roomAvatarUrl, formatBytes, toast } from "../utils/dom";
import { timeFmt, dayFmt, chatKey, sameChat } from "../utils/format";
import { renderRichText } from "../rich-text";
import { attachmentNode } from "../components/attachments";
import { openManageGroupModal } from "../components/modals";
import { ChatComposer } from "../components/composer/ChatComposer";
import { renderSidebar } from "./sidebar";

const TYPING_IDLE_MS = 2000;
const TYPING_EXPIRE_MS = 6000;

interface UploadEntry {
  id: number;
  name: string;
  size: number;
  pct: number;
  xhr: XMLHttpRequest;
  chat: ActiveChat;
  key: string;
}

let uploadSeq = 0;
const uploads = new Map<number, UploadEntry>();

export function typersOf(chat: ActiveChat): string[] {
  return [...(state.typing.get(chatKey(chat))?.keys() ?? [])];
}

export function startTyping(): void {
  if (!state.active) return;
  if (!sameChat(state.typingSentFor, state.active)) {
    stopTyping();
    state.typingSentFor = state.active;
    send({ type: "typing", chat: state.active, active: true });
  }
  if (state.typingTimer) clearTimeout(state.typingTimer);
  state.typingTimer = setTimeout(stopTyping, TYPING_IDLE_MS);
}

export function stopTyping(): void {
  if (state.typingTimer) clearTimeout(state.typingTimer);
  if (state.typingSentFor) {
    send({ type: "typing", chat: state.typingSentFor, active: false });
    state.typingSentFor = null;
  }
}

export function setTyping(chat: ActiveChat, from: string): void {
  const key = chatKey(chat);
  if (!state.typing.has(key)) state.typing.set(key, new Map());
  const people = state.typing.get(key)!;
  const existing = people.get(from);
  if (existing) clearTimeout(existing);
  people.set(
    from,
    setTimeout(() => clearTyping(chat, from), TYPING_EXPIRE_MS)
  );
  renderTyping();
  renderSidebar();
}

export function clearTyping(chat: ActiveChat, from: string): void {
  const people = state.typing.get(chatKey(chat));
  if (!people?.has(from)) return;
  const timer = people.get(from);
  if (timer) clearTimeout(timer);
  people.delete(from);
  renderTyping();
  renderSidebar();
}

export function openChat(chat: ActiveChat): void {
  stopTyping();
  state.active = chat;
  const cs = chatState(chat);
  cs.unread = 0;
  if (chat.kind === "dm") state.dmPeers.add(chat.id);
  if (!cs.loaded) requestHistory(chat);

  if (chat.kind === "room" && state.adminRooms.has(chat.id)) {
    send({ type: "get_room_details", room: chat.id });
  }

  ChatComposer.setActiveChat(chat);

  const drawer = $("drawer") as HTMLInputElement | null;
  if (drawer) drawer.checked = false;

  render();
}

export function requestHistory(chat: ActiveChat): void {
  send({ type: "history", chat });
}

export function leaveRoom(room: string): void {
  send({ type: "leave", room });
  state.joined.delete(room);
  if (sameChat(state.active, { kind: "room", id: room })) {
    state.active = null;
    ChatComposer.setActiveChat(null);
  }
  render();
}

export function renderHeader(): void {
  const head = $("chat-head");
  if (!head) return;

  const chat = state.active;
  if (!chat) {
    head.replaceChildren(h("span", { class: "text-base-content/60" }, "No chat selected"));
    return;
  }

  const isRoom = chat.kind === "room";
  const online = state.users.includes(chat.id);
  const isAdmin = isRoom && state.adminRooms.has(chat.id);
  const pendingCount =
    (state.activeRoomDetails?.room === chat.id && state.activeRoomDetails?.requests?.length) || 0;

  const left = h(
    "div",
    { class: "flex items-center gap-3 min-w-0 flex-1" },
    avatar(isRoom ? roomAvatarUrl(chat.id) : avatarUrl(chat.id), "w-10", isRoom ? null : online),
    h(
      "div",
      { class: "min-w-0" },
      h(
        "div",
        { class: "font-semibold truncate flex items-center gap-2" },
        isRoom ? `# ${chat.id}` : `@${chat.id}`,
        isAdmin ? h("span", { class: "badge badge-xs badge-primary font-bold" }, "ADMIN") : null
      ),
      h(
        "div",
        { class: "text-xs text-base-content/60" },
        isRoom
          ? isAdmin
            ? "Group admin"
            : "Group chat"
          : online
          ? "Online"
          : "Offline — they'll see it later"
      )
    )
  );

  const nodes: HTMLElement[] = [left];

  if (isAdmin) {
    nodes.push(
      h(
        "button",
        {
          type: "button",
          class: "btn btn-sm btn-outline btn-primary gap-1 ml-auto",
          onclick: () => openManageGroupModal(chat.id),
        },
        "🛡️ Manage",
        pendingCount > 0
          ? h("span", { class: "badge badge-xs badge-error text-white font-bold" }, pendingCount)
          : null
      )
    );
  }

  head.replaceChildren(...nodes);
}

export function emptyState(text: string): HTMLElement {
  return h("div", { class: "flex h-full items-center justify-center text-base-content/50" }, text);
}

export function messageNodes(m: ChatMessage, prev?: ChatMessage): HTMLElement[] {
  const nodes: HTMLElement[] = [];
  const date = new Date(m.ts);

  if (!prev || new Date(prev.ts).toDateString() !== date.toDateString()) {
    nodes.push(h("div", { class: "divider text-xs text-base-content/50" }, dayFmt.format(date)));
  }

  const mine = m.from === state.me;
  const grouped = prev && prev.from === m.from && m.ts - prev.ts < 5 * 60 * 1000;

  nodes.push(
    h(
      "div",
      { class: `chat ${mine ? "chat-end" : "chat-start"}` },
      h(
        "div",
        { class: "chat-image avatar" },
        h(
          "div",
          { class: `w-10 rounded-full bg-base-300 ${grouped ? "invisible" : ""}` },
          h("img", { src: avatarUrl(m.from), alt: m.from })
        )
      ),
      grouped
        ? null
        : h(
            "div",
            { class: "chat-header" },
            mine ? "You" : `@${m.from}`,
            h("time", { class: "text-xs opacity-50 ml-1" }, timeFmt.format(date))
          ),
      h(
        "div",
        {
          class: `chat-bubble max-w-[min(75%,36rem)] break-words ${
            mine ? "chat-bubble-primary" : "chat-bubble-incoming"
          }`,
        },
        m.attachment ? attachmentNode(m.attachment) : null,
        m.text ? h("div", { class: m.attachment ? "pt-1" : "" }, renderRichText(m.text)) : null
      )
    )
  );

  return nodes;
}

export function appendMessage(message: ChatMessage, previous?: ChatMessage): void {
  const box = $("messages");
  if (!box) return;
  if (!previous) box.replaceChildren();
  const nearBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 120;
  box.append(...messageNodes(message, previous));
  if (nearBottom || message.from === state.me) {
    box.scrollTop = box.scrollHeight;
  }
}

export function renderMessages(): void {
  const box = $("messages");
  if (!box) return;

  const chat = state.active;
  if (!chat) {
    box.replaceChildren(emptyState("Pick a group or a person on the left."));
    return;
  }

  const cs = chatState(chat);
  if (!cs.loaded && cs.messages.length === 0) {
    box.replaceChildren(
      h(
        "div",
        { class: "flex h-full items-center justify-center" },
        h("span", { class: "loading loading-spinner loading-lg text-primary" })
      )
    );
    return;
  }

  if (cs.messages.length === 0) {
    box.replaceChildren(emptyState("No messages yet. Say hi 👋"));
    return;
  }

  box.replaceChildren(...cs.messages.flatMap((m, i) => messageNodes(m, cs.messages[i - 1])));
  box.scrollTop = box.scrollHeight;
}

export function renderTyping(): void {
  const el = $("typing");
  if (!el) return;

  const typers = state.active ? typersOf(state.active) : [];
  if (!typers.length) {
    el.replaceChildren();
    return;
  }

  const who =
    typers.length === 1
      ? `@${typers[0]} is typing`
      : typers.length === 2
      ? `@${typers[0]} and @${typers[1]} are typing`
      : `${typers.length} people are typing`;

  el.replaceChildren(
    h(
      "div",
      { class: "avatar-group -space-x-3" },
      ...typers.slice(0, 3).map((n) => avatar(avatarUrl(n), "w-5"))
    ),
    h("span", {}, who),
    h("span", { class: "loading loading-dots loading-xs" })
  );
}

export function renderUploads(): void {
  const el = $("uploads");
  if (!el) return;

  const activeKey = state.active ? chatKey(state.active) : null;
  const items = [...uploads.values()].filter((u) => u.key === activeKey);

  el.replaceChildren(
    ...items.map((u) =>
      h(
        "div",
        { class: "flex items-center gap-3 rounded-lg bg-base-200 p-2" },
        h("span", { class: "text-xl" }, "📤"),
        h(
          "div",
          { class: "min-w-0 flex-1" },
          h(
            "div",
            { class: "flex justify-between gap-2 text-xs" },
            h("span", { class: "truncate font-medium" }, u.name),
            h("span", { class: "opacity-70" }, `${u.pct}% of ${formatBytes(u.size)}`)
          ),
          h("progress", {
            class: "progress progress-primary h-2 w-full",
            value: String(u.pct),
            max: "100",
          })
        ),
        h(
          "button",
          { type: "button", class: "btn btn-xs btn-ghost", onclick: () => cancelUpload(u.id) },
          "Cancel"
        )
      )
    )
  );
}

export function cancelUpload(id: number): void {
  uploads.get(id)?.xhr.abort();
}

export function uploadFile(file: File, chat: ActiveChat): void {
  const id = ++uploadSeq;
  const xhr = new XMLHttpRequest();
  const entry: UploadEntry = {
    id,
    name: file.name,
    size: file.size,
    pct: 0,
    xhr,
    chat,
    key: chatKey(chat),
  };
  uploads.set(id, entry);
  renderUploads();

  const finish = () => {
    uploads.delete(id);
    renderUploads();
  };

  xhr.upload.addEventListener("progress", (e) => {
    if (!e.lengthComputable) return;
    entry.pct = Math.round((e.loaded / e.total) * 100);
    renderUploads();
  });

  xhr.addEventListener("load", () => {
    finish();
    let body: any = null;
    try {
      body = JSON.parse(xhr.responseText);
    } catch {
      body = null;
    }
    if (xhr.status !== 200 || !body?.id) {
      toast(body?.error ?? `Could not send ${file.name} (${xhr.status}).`);
      return;
    }
    send({ type: "send", chat: entry.chat, text: "", attachment: body });
  });

  xhr.addEventListener("error", () => {
    finish();
    toast(`Could not send ${file.name}. Check the connection.`);
  });

  xhr.addEventListener("abort", finish);

  const query = new URLSearchParams({ name: state.me ?? "", token: state.token ?? "" });
  xhr.open("POST", `/api/upload?${query}`);
  const form = new FormData();
  form.append("file", file, file.name);
  xhr.send(form);
}

export function render(): void {
  renderSidebar();
  renderHeader();
  renderMessages();
  renderTyping();
  renderUploads();
}
