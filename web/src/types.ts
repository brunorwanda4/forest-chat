export type ChatKind = "room" | "dm";

export interface ActiveChat {
  kind: ChatKind;
  id: string;
}

export interface MemberInfo {
  username: string;
  role: "creator" | "admin" | "member" | string;
  joined_ts: number;
}

export interface RequestInfo {
  username: string;
  created_ts: number;
}

export interface RoomDetails {
  room: string;
  is_admin: boolean;
  members: MemberInfo[];
  requests: RequestInfo[];
}

export interface ChatAttachment {
  id: string;
  name: string;
  size: number;
  mime: string;
}

export interface ChatMessage {
  id?: string;
  room?: string;
  from: string;
  to?: string;
  text: string;
  ts: number;
  attachment?: ChatAttachment | null;
}

export interface AttachmentItem {
  id: string;
  file: File;
  name: string;
  size: number;
  mime: string;
  previewUrl: string | null;
}

export interface SavedAccount {
  name: string;
  token: string;
  savedAt: number;
}

export interface ChatEntry {
  messages: ChatMessage[];
  loaded: boolean;
  unread: number;
}

export interface AppState {
  me: string | null;
  token: string | null;
  ws: WebSocket | null;
  retry: any;
  users: string[];
  rooms: string[];
  joined: Set<string>;
  pending: Set<string>;
  adminRooms: Set<string>;
  activeRoomDetails: RoomDetails | null;
  dmPeers: Set<string>;
  active: ActiveChat | null;
  chats: Map<string, ChatEntry>;
  typing: Map<string, Map<string, any>>;
  typingSentFor: ActiveChat | null;
  typingTimer: any;
  authMode: "login" | "register";
  pendingJoinRoom: string | null;
}

export interface LanParticipant {
  peer_id: string;
  name: string;
  is_host: boolean;
}

export interface LanRoomInfo {
  room_id: string;
  is_host: boolean;
  peer_id: string;
  local_ip: string;
  join_address: string;
  participants: LanParticipant[];
}

export interface LanMeetingState {
  inMeeting: boolean;
  isHost: boolean;
  roomId: string | null;
  peerId: string | null;
  localIp: string | null;
  joinAddress: string | null;
  isMuted: boolean;
  isSharingScreen: boolean;
  participants: LanParticipant[];
  presenterPeerId: string | null;
  speakingPeers: Set<string>;
  mutedPeers: Set<string>;
}

export interface EmojiItem {
  emoji: string;
  name?: string;
  shortcode?: string;
  group?: string;
}

export type ServerMsg =
  | {
      type: "welcome";
      me: string;
      users: string[];
      rooms: string[];
      joined_rooms: string[];
      pending_rooms: string[];
      admin_rooms: string[];
    }
  | { type: "presence"; users: string[] }
  | {
      type: "rooms";
      rooms: string[];
      joined_rooms: string[];
      pending_rooms: string[];
      admin_rooms: string[];
    }
  | { type: "joined"; room: string }
  | {
      type: "room_details";
      room: string;
      is_admin: boolean;
      members: MemberInfo[];
      requests: RequestInfo[];
    }
  | { type: "join_requested"; room: string; user: string }
  | { type: "join_approved"; room: string; user: string }
  | { type: "join_rejected"; room: string; user: string }
  | { type: "admin_promoted"; room: string; user: string }
  | { type: "history"; chat: ActiveChat; messages: ChatMessage[] }
  | {
      type: "message";
      chat: ActiveChat;
      from: string;
      text: string;
      ts: number;
      attachment?: ChatAttachment | null;
    }
  | { type: "typing"; chat: ActiveChat; from: string; active: boolean }
  | { type: "error"; message: string }
  | { type: string; [key: string]: any };
