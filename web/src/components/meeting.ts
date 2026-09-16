import type { LanMeetingState, LanParticipant, LanRoomInfo } from "../types";
import { state } from "../state";
import { $, h, avatarUrl, toast } from "../utils/dom";
import { timeFmt } from "../utils/format";
import { tauriInvoke } from "./attachments";

export const meetingState: LanMeetingState = {
  inMeeting: false,
  isHost: false,
  roomId: null,
  peerId: null,
  localIp: null,
  joinAddress: null,
  isMuted: false,
  isSharingScreen: false,
  participants: [],
  presenterPeerId: null,
  speakingPeers: new Set(),
  mutedPeers: new Set(),
};

export async function onTauriEvent(
  eventName: string,
  handler: (payload: any) => void
): Promise<(() => void) | undefined> {
  const win = window as any;
  if (win.__TAURI__?.event?.listen) {
    return await win.__TAURI__.event.listen(eventName, (e: any) => handler(e.payload));
  }
  if (win.__TAURI_INTERNALS__?.invoke) {
    const handlerId = win.__TAURI_INTERNALS__.transformCallback((event: any) => {
      handler(event.payload);
    });
    try {
      await win.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
        event: eventName,
        target: { kind: "Any" },
        handler: handlerId,
      });
      return () => {
        win.__TAURI_INTERNALS__.invoke("plugin:event|unlisten", {
          event: eventName,
          eventId: handlerId,
        });
      };
    } catch (e) {
      console.warn("Direct event listen fallback error:", e);
    }
  }
  return undefined;
}

export function switchAppMode(mode: "chat" | "meeting"): void {
  const chatPane = $("chat-pane");
  const meetingPane = $("meeting-pane");
  const navChatBtn = $("nav-chat-btn");
  const navMeetingBtn = $("nav-meeting-btn");

  if (mode === "chat") {
    chatPane?.classList.remove("hidden");
    meetingPane?.classList.add("hidden");
    if (navChatBtn) navChatBtn.className = "join-item btn btn-sm btn-primary";
    if (navMeetingBtn) navMeetingBtn.className = "join-item btn btn-sm btn-ghost gap-1";
  } else {
    chatPane?.classList.add("hidden");
    meetingPane?.classList.remove("hidden");
    if (navChatBtn) navChatBtn.className = "join-item btn btn-sm btn-ghost";
    if (navMeetingBtn) navMeetingBtn.className = "join-item btn btn-sm btn-primary gap-1";
    loadLocalIpInfo();
    if (state.me) {
      const hostInput = $("host-name") as HTMLInputElement | null;
      const joinInput = $("join-name") as HTMLInputElement | null;
      if (hostInput && !hostInput.value) hostInput.value = state.me;
      if (joinInput && !joinInput.value) joinInput.value = state.me;
    }
  }
}

export async function loadLocalIpInfo(): Promise<void> {
  try {
    const info = await tauriInvoke<{ local_ip?: string }>("get_lan_ip_info");
    if (info?.local_ip) {
      const el = $("host-detected-ip");
      if (el) el.textContent = info.local_ip;
      meetingState.localIp = info.local_ip;
    }
  } catch (e) {
    console.warn("Could not retrieve local IP:", e);
    const el = $("host-detected-ip");
    if (el) el.textContent = "127.0.0.1";
  }
}

export function enterMeetingView(roomInfo: LanRoomInfo): void {
  meetingState.inMeeting = true;
  meetingState.isHost = roomInfo.is_host;
  meetingState.roomId = roomInfo.room_id;
  meetingState.peerId = roomInfo.peer_id;
  meetingState.localIp = roomInfo.local_ip;
  meetingState.joinAddress = roomInfo.join_address;
  meetingState.participants = roomInfo.participants || [];
  meetingState.isMuted = false;
  meetingState.isSharingScreen = false;
  meetingState.presenterPeerId = null;
  meetingState.speakingPeers.clear();
  meetingState.mutedPeers.clear();

  const codeBadge = $("meeting-code-badge");
  const ipBadge = $("meeting-ip-badge");
  if (codeBadge) codeBadge.textContent = `ROOM: #${roomInfo.room_id.toUpperCase()}`;
  if (ipBadge) ipBadge.textContent = `LAN: ${roomInfo.local_ip}`;

  updateParticipantsUI();

  $("screen-placeholder")?.classList.remove("hidden");
  $("meeting-screen-canvas")?.classList.add("hidden");
  $("presenter-pill")?.classList.add("hidden");
  $("meeting-chat-messages")?.replaceChildren();

  updateMicButtonUI();
  updateScreenShareButtonUI();

  $("meeting-lobby")?.classList.add("hidden");
  $("meeting-active")?.classList.remove("hidden");
  toast(`Joined room #${roomInfo.room_id.toUpperCase()}`, "success");
}

export function exitMeetingView(): void {
  meetingState.inMeeting = false;
  meetingState.roomId = null;
  meetingState.peerId = null;
  meetingState.participants = [];
  meetingState.isSharingScreen = false;
  meetingState.presenterPeerId = null;

  $("meeting-active")?.classList.add("hidden");
  $("meeting-lobby")?.classList.remove("hidden");
}

export function updateParticipantsUI(): void {
  const list = $("meeting-participants-list");
  if (!list) return;
  list.replaceChildren();

  const count = meetingState.participants.length;
  const peerCount = $("meeting-peer-count");
  if (peerCount) peerCount.textContent = `${count} ${count === 1 ? "peer" : "peers"}`;

  for (const p of meetingState.participants) {
    const isMe = p.peer_id === meetingState.peerId;
    const isSpeaking = meetingState.speakingPeers.has(p.peer_id);
    const isMuted = meetingState.mutedPeers.has(p.peer_id) || (isMe && meetingState.isMuted);
    const isPresenting =
      meetingState.presenterPeerId === p.peer_id || (isMe && meetingState.isSharingScreen);

    const item = h(
      "div",
      {
        class:
          "flex items-center justify-between p-2 rounded-xl bg-base-200 border border-base-300 gap-2",
      },
      h(
        "div",
        { class: "flex items-center gap-2 min-w-0" },
        h(
          "div",
          {
            class: `avatar ${
              isSpeaking ? "ring ring-success ring-offset-base-100 ring-offset-1" : ""
            }`,
          },
          h(
            "div",
            { class: "w-8 rounded-full bg-base-300" },
            h("img", { src: avatarUrl(p.name), alt: p.name })
          )
        ),
        h(
          "div",
          { class: "min-w-0" },
          h(
            "div",
            { class: "flex items-center gap-1" },
            h("span", { class: "font-semibold text-xs truncate" }, p.name),
            isMe ? h("span", { class: "badge badge-xs badge-neutral" }, "You") : null,
            p.is_host ? h("span", { class: "badge badge-xs badge-primary" }, "Host") : null
          ),
          isPresenting
            ? h(
                "span",
                { class: "badge badge-xs badge-warning text-[10px]" },
                "Screen Sharing"
              )
            : null
        )
      ),
      h(
        "div",
        { class: "flex items-center gap-1 text-base-content/70" },
        isMuted
          ? h(
              "span",
              { class: "text-error text-xs p-1 rounded-full bg-error/10" },
              h(
                "svg",
                {
                  xmlns: "http://www.w3.org/2000/svg",
                  class: "h-4 w-4",
                  fill: "none",
                  viewBox: "0 0 24 24",
                  stroke: "currentColor",
                },
                h("path", {
                  "stroke-linecap": "round",
                  "stroke-linejoin": "round",
                  "stroke-width": "2",
                  d: "M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z",
                }),
                h("path", {
                  "stroke-linecap": "round",
                  "stroke-linejoin": "round",
                  "stroke-width": "2",
                  d: "M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2",
                })
              )
            )
          : h(
              "span",
              { class: `${isSpeaking ? "text-success" : "text-base-content/40"} text-xs p-1` },
              h(
                "svg",
                {
                  xmlns: "http://www.w3.org/2000/svg",
                  class: "h-4 w-4",
                  fill: "none",
                  viewBox: "0 0 24 24",
                  stroke: "currentColor",
                },
                h("path", {
                  "stroke-linecap": "round",
                  "stroke-linejoin": "round",
                  "stroke-width": "2",
                  d: "M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z",
                })
              )
            )
      )
    );
    list.append(item);
  }
}

export function updateMicButtonUI(): void {
  const btn = $("btn-meeting-mic");
  const iconOn = $("icon-mic-on");
  const iconOff = $("icon-mic-off");
  const label = $("label-meeting-mic");

  if (meetingState.isMuted) {
    iconOn?.classList.add("hidden");
    iconOff?.classList.remove("hidden");
    if (label) label.textContent = "Unmute";
    if (btn) btn.className = "btn btn-sm md:btn-md gap-2 btn-error text-error-content";
  } else {
    iconOn?.classList.remove("hidden");
    iconOff?.classList.add("hidden");
    if (label) label.textContent = "Mute";
    if (btn) btn.className = "btn btn-sm md:btn-md gap-2 btn-ghost border border-base-300";
  }
}

export function updateScreenShareButtonUI(): void {
  const btn = $("btn-meeting-share");
  const label = $("label-meeting-share");

  if (meetingState.isSharingScreen) {
    if (label) label.textContent = "Stop Share";
    if (btn) btn.className = "btn btn-sm md:btn-md gap-2 btn-warning text-warning-content";
  } else {
    if (label) label.textContent = "Share Screen";
    if (btn) btn.className = "btn btn-sm md:btn-md gap-2 btn-ghost border border-base-300";
  }
}

export function initMeetingUI(): void {
  $("nav-chat-btn")?.addEventListener("click", () => switchAppMode("chat"));
  $("nav-meeting-btn")?.addEventListener("click", () => switchAppMode("meeting"));

  // Lobby Tab Switching
  $("tab-host")?.addEventListener("click", () => {
    const tabHost = $("tab-host");
    const tabJoin = $("tab-join");
    if (tabHost) tabHost.className = "tab tab-active font-semibold";
    if (tabJoin) tabJoin.className = "tab font-semibold";
    $("form-host")?.classList.remove("hidden");
    $("form-join")?.classList.add("hidden");
  });

  $("tab-join")?.addEventListener("click", () => {
    const tabJoin = $("tab-join");
    const tabHost = $("tab-host");
    if (tabJoin) tabJoin.className = "tab tab-active font-semibold";
    if (tabHost) tabHost.className = "tab font-semibold";
    $("form-join")?.classList.remove("hidden");
    $("form-host")?.classList.add("hidden");
  });

  // Side Panel Tabs
  $("side-tab-participants")?.addEventListener("click", () => {
    const tabPart = $("side-tab-participants");
    const tabChat = $("side-tab-chat");
    if (tabPart) tabPart.className = "tab tab-active text-xs font-semibold py-2";
    if (tabChat) tabChat.className = "tab text-xs font-semibold py-2";
    $("panel-participants")?.classList.remove("hidden");
    $("panel-chat")?.classList.add("hidden");
  });

  $("side-tab-chat")?.addEventListener("click", () => {
    const tabChat = $("side-tab-chat");
    const tabPart = $("side-tab-participants");
    if (tabChat) tabChat.className = "tab tab-active text-xs font-semibold py-2";
    if (tabPart) tabPart.className = "tab text-xs font-semibold py-2";
    $("panel-chat")?.classList.remove("hidden");
    $("panel-participants")?.classList.add("hidden");
    const msgContainer = $("meeting-chat-messages");
    if (msgContainer) msgContainer.scrollTop = msgContainer.scrollHeight;
  });

  // Form Submissions
  $("form-host")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const name = (($("host-name") as HTMLInputElement | null)?.value || "").trim();
    const port = parseInt(($("host-port") as HTMLInputElement | null)?.value || "8080", 10) || 8080;
    if (!name) return toast("Please enter your display name");

    const btn = $("btn-start-host") as HTMLButtonElement | null;
    if (btn) btn.disabled = true;
    try {
      const res = await tauriInvoke<LanRoomInfo>("create_lan_meeting", {
        name,
        customPort: port,
      });
      enterMeetingView(res);
    } catch (err: any) {
      toast(`Host error: ${err}`, "error");
    } finally {
      if (btn) btn.disabled = false;
    }
  });

  $("form-join")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const name = (($("join-name") as HTMLInputElement | null)?.value || "").trim();
    const joinUrlOrIp = (($("join-host-addr") as HTMLInputElement | null)?.value || "").trim();
    const roomId = (($("join-room-code") as HTMLInputElement | null)?.value || "").trim().toLowerCase();

    if (!name || !joinUrlOrIp || !roomId) {
      return toast("Please fill in all fields");
    }

    const btn = $("btn-start-join") as HTMLButtonElement | null;
    if (btn) btn.disabled = true;
    try {
      const res = await tauriInvoke<LanRoomInfo>("join_lan_meeting", {
        joinUrlOrIp,
        roomId,
        name,
      });
      enterMeetingView(res);
    } catch (err: any) {
      toast(`Join error: ${err}`, "error");
    } finally {
      if (btn) btn.disabled = false;
    }
  });

  // Meeting Controls
  $("btn-copy-join")?.addEventListener("click", () => {
    if (meetingState.joinAddress) {
      navigator.clipboard
        .writeText(meetingState.joinAddress)
        .then(() => toast("Join link copied to clipboard!", "success"))
        .catch(() => toast(meetingState.joinAddress!, "info"));
    }
  });

  $("btn-meeting-mic")?.addEventListener("click", async () => {
    try {
      const newMuted = await tauriInvoke<boolean>("toggle_meeting_mic");
      meetingState.isMuted = newMuted;
      updateMicButtonUI();
      updateParticipantsUI();
    } catch (err: any) {
      toast(`Mic toggle error: ${err}`, "error");
    }
  });

  $("btn-meeting-share")?.addEventListener("click", async () => {
    try {
      const newSharing = await tauriInvoke<boolean>("toggle_meeting_screen_share");
      meetingState.isSharingScreen = newSharing;
      updateScreenShareButtonUI();
      updateParticipantsUI();

      if (!newSharing && meetingState.presenterPeerId === meetingState.peerId) {
        $("screen-placeholder")?.classList.remove("hidden");
        $("meeting-screen-canvas")?.classList.add("hidden");
        $("presenter-pill")?.classList.add("hidden");
      }
    } catch (err: any) {
      toast(`Screen share error: ${err}`, "error");
    }
  });

  $("btn-meeting-leave")?.addEventListener("click", async () => {
    try {
      await tauriInvoke("leave_lan_meeting");
      exitMeetingView();
      toast("Left meeting room", "info");
    } catch (err: any) {
      toast(`Leave error: ${err}`, "error");
      exitMeetingView();
    }
  });

  $("meeting-chat-form")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const input = $("meeting-chat-input") as HTMLInputElement | null;
    const text = (input?.value || "").trim();
    if (!text) return;
    try {
      await tauriInvoke("send_meeting_chat", { text });
      if (input) input.value = "";
    } catch (err: any) {
      toast(`Chat error: ${err}`, "error");
    }
  });

  setupMeetingCanvasAndEvents();
}

function setupMeetingCanvasAndEvents(): void {
  const screenCanvas = $("meeting-screen-canvas") as HTMLCanvasElement | null;
  const screenCtx = screenCanvas?.getContext("2d");
  const screenImg = new Image();

  screenImg.onload = () => {
    if (!screenCanvas || !screenCtx) return;
    if (screenCanvas.width !== screenImg.width || screenCanvas.height !== screenImg.height) {
      screenCanvas.width = screenImg.width;
      screenCanvas.height = screenImg.height;
    }
    screenCtx.drawImage(screenImg, 0, 0);
  };

  onTauriEvent("meeting://screen-frame", (payload: any) => {
    if (payload?.data_url) {
      $("screen-placeholder")?.classList.add("hidden");
      $("meeting-screen-canvas")?.classList.remove("hidden");
      screenImg.src = payload.data_url;
    }
  });

  onTauriEvent("meeting://screen-share-started", (payload: any) => {
    meetingState.presenterPeerId = payload?.peer_id;
    const presenter = meetingState.participants.find((p) => p.peer_id === payload?.peer_id);
    const name = presenter ? presenter.name : payload?.name || "Someone";
    const nameEl = $("presenter-name");
    if (nameEl) nameEl.textContent = `${name} is sharing screen`;
    $("presenter-pill")?.classList.remove("hidden");
    updateParticipantsUI();
  });

  onTauriEvent("meeting://screen-share-stopped", (payload: any) => {
    if (meetingState.presenterPeerId === payload?.peer_id) {
      meetingState.presenterPeerId = null;
      $("meeting-screen-canvas")?.classList.add("hidden");
      $("screen-placeholder")?.classList.remove("hidden");
      $("presenter-pill")?.classList.add("hidden");
      updateParticipantsUI();
    }
  });

  onTauriEvent("meeting://participants-update", (participants: LanParticipant[]) => {
    meetingState.participants = participants || [];
    updateParticipantsUI();
  });

  onTauriEvent("meeting://participant-left", (peerId: string) => {
    const leftPeer = meetingState.participants.find((p) => p.peer_id === peerId);
    if (leftPeer) {
      toast(`${leftPeer.name} left the room`, "info");
    }
    meetingState.participants = meetingState.participants.filter((p) => p.peer_id !== peerId);
    if (meetingState.presenterPeerId === peerId) {
      meetingState.presenterPeerId = null;
      $("meeting-screen-canvas")?.classList.add("hidden");
      $("screen-placeholder")?.classList.remove("hidden");
      $("presenter-pill")?.classList.add("hidden");
    }
    updateParticipantsUI();
  });

  onTauriEvent("meeting://chat-message", (chat: any) => {
    const isMine = chat.sender_id === meetingState.peerId;
    const container = $("meeting-chat-messages");
    if (!container) return;

    const bubble = h(
      "div",
      { class: `chat ${isMine ? "chat-end" : "chat-start"}` },
      h(
        "div",
        { class: "chat-image avatar" },
        h(
          "div",
          { class: "w-7 rounded-full bg-base-300" },
          h("img", { src: avatarUrl(chat.sender_name), alt: chat.sender_name })
        )
      ),
      h(
        "div",
        { class: "chat-header text-[11px] opacity-70 mb-0.5" },
        isMine ? "You" : chat.sender_name,
        h(
          "time",
          { class: "text-[10px] opacity-50 ml-1" },
          timeFmt.format(new Date(chat.timestamp))
        )
      ),
      h(
        "div",
        {
          class: `chat-bubble chat-bubble-sm text-xs break-words ${
            isMine ? "chat-bubble-primary" : "chat-bubble-incoming"
          }`,
        },
        chat.text
      )
    );

    container.append(bubble);
    container.scrollTop = container.scrollHeight;
  });

  onTauriEvent("meeting://speaking", (payload: any) => {
    if (payload?.is_speaking) {
      meetingState.speakingPeers.add(payload.peer_id);
    } else {
      meetingState.speakingPeers.delete(payload.peer_id);
    }
    updateParticipantsUI();
  });

  onTauriEvent("meeting://mic-status", (payload: any) => {
    if (payload?.is_muted) {
      meetingState.mutedPeers.add(payload.peer_id);
    } else {
      meetingState.mutedPeers.delete(payload.peer_id);
    }
    updateParticipantsUI();
  });

  onTauriEvent("meeting://error", (msg: any) => {
    toast(String(msg), "error");
  });

  onTauriEvent("meeting://left", () => {
    exitMeetingView();
  });
}
