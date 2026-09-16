import { $ } from "./utils/dom";
import { state, send, getActiveAccount, ACTIVE_KEY } from "./state";
import { apiVerify } from "./api";
import { connect } from "./socket";
import { setConnectCallback, initAuthUI, updateLoginAvatar, showLogin } from "./views/auth";
import { setRenderRoomsCallback, initGroupModals } from "./components/modals";
import { setOpenDmHandler } from "./rich-text";
import {
  ChatComposer,
  setSendCallback,
  setTypingCallbacks,
} from "./components/composer/ChatComposer";
import { initViewerModalEvents } from "./components/attachments";
import { initMeetingUI } from "./components/meeting";
import { openChat, startTyping, stopTyping } from "./views/chat";
import { renderRooms } from "./views/sidebar";

// Expose $ globally for any modal buttons using inline onclick="$('id').close()"
(window as any).$ = $;

export async function boot(): Promise<void> {
  // Wire callbacks
  setConnectCallback(connect);
  setRenderRoomsCallback(renderRooms);
  setOpenDmHandler((username: string) => openChat({ kind: "dm", id: username }));
  setSendCallback((params) => send(params));
  setTypingCallbacks(startTyping, stopTyping);

  // Initialize UI components
  ChatComposer.init();
  initAuthUI();
  initGroupModals();
  initViewerModalEvents();
  initMeetingUI();

  updateLoginAvatar();

  const active = getActiveAccount();
  if (active?.name && active?.token) {
    try {
      const data = await apiVerify(active.name, active.token);
      if (data.valid) {
        connect(active.name, active.token);
        return;
      }
    } catch (_) {
      // Fall through to showLogin
    }
    localStorage.removeItem(ACTIVE_KEY);
  }
  showLogin();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => boot());
} else {
  boot();
}
