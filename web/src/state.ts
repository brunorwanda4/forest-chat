import type { ActiveChat, AppState, ChatEntry, SavedAccount } from "./types";
import { chatKey } from "./utils/format";

export const TYPING_IDLE_MS = 2000;
export const TYPING_EXPIRE_MS = 6000;

export const ACCOUNTS_KEY = "forest-chat:saved-accounts";
export const ACTIVE_KEY = "forest-chat:active-account";

export const state: AppState = {
  me: null,
  token: null,
  ws: null,
  retry: null,
  users: [],
  rooms: [],
  joined: new Set(),
  pending: new Set(),
  adminRooms: new Set(),
  activeRoomDetails: null,
  dmPeers: new Set(),
  active: null,
  chats: new Map(),
  typing: new Map(),
  typingSentFor: null,
  typingTimer: null,
  authMode: "login",
  pendingJoinRoom: null,
};

export function chatState(chat: ActiveChat): ChatEntry {
  const key = chatKey(chat);
  if (!state.chats.has(key)) {
    state.chats.set(key, { messages: [], loaded: false, unread: 0 });
  }
  return state.chats.get(key)!;
}

export function send(payload: Record<string, any>): void {
  if (state.ws?.readyState === WebSocket.OPEN) {
    state.ws.send(JSON.stringify(payload));
  }
}

export function getSavedAccounts(): SavedAccount[] {
  try {
    return JSON.parse(localStorage.getItem(ACCOUNTS_KEY) || "[]");
  } catch (_) {
    return [];
  }
}

export function saveAccount(name: string, token: string): void {
  const accounts = getSavedAccounts().filter((a) => a.name !== name);
  accounts.unshift({ name, token, savedAt: Date.now() });
  try {
    localStorage.setItem(ACCOUNTS_KEY, JSON.stringify(accounts.slice(0, 10)));
  } catch (_) {}
}

export function forgetAccount(name: string): void {
  const accounts = getSavedAccounts().filter((a) => a.name !== name);
  try {
    localStorage.setItem(ACCOUNTS_KEY, JSON.stringify(accounts));
  } catch (_) {}
  const active = getActiveAccount();
  if (active?.name === name) {
    clearActiveAccount();
  }
}

export function getActiveAccount(): { name: string; token: string } | null {
  try {
    const raw = localStorage.getItem(ACTIVE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch (_) {
    return null;
  }
}

export function setActiveAccount(name: string, token: string): void {
  try {
    localStorage.setItem(ACTIVE_KEY, JSON.stringify({ name, token }));
  } catch (_) {}
}

export function clearActiveAccount(): void {
  try {
    localStorage.removeItem(ACTIVE_KEY);
  } catch (_) {}
}
