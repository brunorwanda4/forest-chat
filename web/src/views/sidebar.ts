import type { ActiveChat } from "../types";
import { state } from "../state";
import { $, h, avatar, avatarUrl, roomAvatarUrl, toast } from "../utils/dom";
import { chatKey, sameChat } from "../utils/format";
import { openRequestJoinModal } from "../components/modals";
import { openChat, leaveRoom, typersOf } from "./chat";

export interface SidebarItemProps {
  chat: ActiveChat;
  img: string;
  online?: boolean | null;
  title: string;
  subtitle?: string | null;
  badge?: HTMLElement | null;
  action?: HTMLElement | null;
}

export function sidebarItem({
  chat,
  img,
  online = null,
  title,
  subtitle,
  badge,
  action,
}: SidebarItemProps): HTMLElement {
  const cs = state.chats.get(chatKey(chat));
  const typers = typersOf(chat);
  const active = sameChat(state.active, chat);

  return h(
    "li",
    {},
    h(
      "a",
      {
        class: `flex items-center gap-3 ${active ? "menu-active" : ""}`,
        onclick: () => openChat(chat),
      },
      avatar(img, "w-9", online),
      h(
        "div",
        { class: "min-w-0 flex-1" },
        h(
          "div",
          { class: "truncate font-medium flex items-center gap-1.5" },
          title,
          badge
        ),
        typers.length
          ? h(
              "div",
              { class: "text-xs text-success flex items-center gap-1" },
              h("span", { class: "loading loading-dots loading-xs" }),
              "typing"
            )
          : subtitle
          ? h("div", { class: "text-xs text-base-content/60 truncate" }, subtitle)
          : null
      ),
      cs?.unread ? h("span", { class: "badge badge-primary badge-sm" }, cs.unread) : null,
      action
    )
  );
}

export function lastLine(chat: ActiveChat): string | null {
  const last = state.chats.get(chatKey(chat))?.messages.at(-1);
  if (!last) return null;
  return `${last.from === state.me ? "You" : last.from}: ${last.text}`;
}

export function renderMe(): void {
  const el = $("me");
  if (!el) return;
  el.replaceChildren(
    avatar(avatarUrl(state.me || "forest"), "w-12", true),
    h(
      "div",
      { class: "min-w-0" },
      h("div", { class: "font-semibold truncate" }, `@${state.me}`),
      h("div", { class: "text-xs text-base-content/60" }, `${state.users.length} online`)
    )
  );
}

export function renderRooms(): void {
  const el = $("rooms");
  if (!el) return;

  const joined = [...state.joined].sort();
  const others = state.rooms.filter((r) => !state.joined.has(r));

  const items: HTMLElement[] = joined.map((room) => {
    const chat: ActiveChat = { kind: "room", id: room };
    const isAdmin = state.adminRooms.has(room);
    return sidebarItem({
      chat,
      img: roomAvatarUrl(room),
      title: `# ${room}`,
      subtitle: lastLine(chat),
      badge: isAdmin ? h("span", { class: "badge badge-xs badge-primary font-bold" }, "ADMIN") : null,
      action:
        room !== "general"
          ? h(
              "button",
              {
                class: "btn btn-ghost btn-xs opacity-60 hover:opacity-100",
                title: "Leave group",
                onclick: (e: MouseEvent) => {
                  e.stopPropagation();
                  leaveRoom(room);
                },
              },
              "✕"
            )
          : null,
    });
  });

  for (const room of others) {
    const isPending = state.pending.has(room);
    items.push(
      h(
        "li",
        {},
        h(
          "a",
          {
            class: "flex items-center gap-3 opacity-80 hover:opacity-100",
            onclick: () => {
              if (isPending) {
                toast(`Your request to join #${room} is waiting for admin approval.`, "info");
              } else {
                openRequestJoinModal(room);
              }
            },
          },
          avatar(roomAvatarUrl(room), "w-9"),
          h(
            "div",
            { class: "flex-1 min-w-0" },
            h("span", { class: "truncate block font-medium" }, `# ${room}`),
            h(
              "span",
              { class: "text-[11px] text-base-content/60 block truncate" },
              isPending ? "Pending approval..." : "Approval required"
            )
          ),
          h(
            "span",
            {
              class: `badge badge-sm ${isPending ? "badge-warning" : "badge-outline badge-secondary"}`,
            },
            isPending ? "Pending" : "Ask to Join"
          )
        )
      )
    );
  }

  el.replaceChildren(...items);
}

export function renderPeople(): void {
  const el = $("people");
  if (!el) return;

  const online = new Set(state.users);
  const peers = new Set([...state.users, ...state.dmPeers]);
  if (state.me) peers.delete(state.me);

  const sorted = [...peers].sort((a, b) => {
    const aOn = online.has(a) ? 1 : 0;
    const bOn = online.has(b) ? 1 : 0;
    return bOn - aOn || a.localeCompare(b);
  });

  const items = sorted.map((name) => {
    const chat: ActiveChat = { kind: "dm", id: name };
    return sidebarItem({
      chat,
      img: avatarUrl(name),
      online: online.has(name),
      title: `@${name}`,
      subtitle: lastLine(chat) ?? (online.has(name) ? "online" : "offline"),
    });
  });

  el.replaceChildren(
    ...(items.length
      ? items
      : [h("li", { class: "px-4 py-2 text-sm text-base-content/50" }, "Nobody else is here yet.")])
  );
}

export function renderSidebar(): void {
  renderMe();
  renderRooms();
  renderPeople();
}
