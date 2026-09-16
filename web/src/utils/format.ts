import type { ActiveChat } from "../types";

export const timeFmt = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
});

export const dayFmt = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  month: "short",
  day: "numeric",
});

export const chatKey = (chat: ActiveChat): string => `${chat.kind}:${chat.id}`;

export const sameChat = (a: ActiveChat | null, b: ActiveChat | null): boolean =>
  Boolean(a && b && a.kind === b.kind && a.id === b.id);

export const NAME_RE = /^[\p{L}\p{N}_-]{1,24}$/u;
export const ROOM_RE = /^[\p{L}\p{N}_-]{1,32}$/u;
