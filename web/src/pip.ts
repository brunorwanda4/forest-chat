/**
 * The floating always-on-top meeting window.
 *
 * It is a second webview over the same local server, so it holds no meeting
 * state of its own: the Rust side emits `meeting://*` events to every window,
 * and the controls here call the same commands the main window does.
 */

interface PipParticipant {
  peer_id: string;
  name: string;
  is_host: boolean;
}

const $ = (id: string) => document.getElementById(id);

const win = window as any;

async function invoke<T = unknown>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (win.__TAURI__?.core?.invoke) return win.__TAURI__.core.invoke(cmd, args);
  if (win.__TAURI_INTERNALS__?.invoke) return win.__TAURI_INTERNALS__.invoke(cmd, args);
  throw new Error("Not running inside the desktop app");
}

async function listen(eventName: string, handler: (payload: any) => void): Promise<void> {
  if (win.__TAURI__?.event?.listen) {
    await win.__TAURI__.event.listen(eventName, (e: any) => handler(e.payload));
    return;
  }
  if (win.__TAURI_INTERNALS__?.invoke) {
    const handlerId = win.__TAURI_INTERNALS__.transformCallback((event: any) => {
      handler(event.payload);
    });
    await win.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
      event: eventName,
      target: { kind: "Any" },
      handler: handlerId,
    });
  }
}

function avatarUrl(name: string): string {
  return `https://api.dicebear.com/7.x/thumbs/svg?seed=${encodeURIComponent(name)}`;
}

function renderPeople(participants: PipParticipant[]): void {
  const people = $("pip-people");
  const count = $("pip-count");
  if (count) count.textContent = String(participants.length);
  if (!people) return;

  people.innerHTML = "";
  for (const person of participants.slice(0, 6)) {
    const wrap = document.createElement("div");
    wrap.className = "flex flex-col items-center gap-1 w-14";
    wrap.innerHTML =
      `<img class="w-8 h-8 rounded-full bg-base-200" src="${avatarUrl(person.name)}" alt="" />` +
      `<span class="text-[10px] truncate w-full text-center text-base-content/70">${person.name}</span>`;
    people.appendChild(wrap);
  }
}

function boot(): void {
  const canvas = $("pip-canvas") as HTMLCanvasElement | null;
  const ctx = canvas?.getContext("2d");
  const frame = new Image();

  frame.onload = () => {
    if (!canvas || !ctx) return;
    if (canvas.width !== frame.width || canvas.height !== frame.height) {
      canvas.width = frame.width;
      canvas.height = frame.height;
    }
    ctx.drawImage(frame, 0, 0);
  };

  listen("meeting://screen-frame", (payload: any) => {
    if (!payload?.data_url) return;
    $("pip-canvas")?.classList.remove("hidden");
    $("pip-people")?.classList.add("hidden");
    frame.src = payload.data_url;
  });

  listen("meeting://screen-share-started", (payload: any) => {
    const pill = $("pip-presenter");
    if (pill) {
      pill.textContent = `${payload?.name || "Someone"} sharing`;
      pill.classList.remove("hidden");
    }
  });

  listen("meeting://screen-share-stopped", () => {
    $("pip-presenter")?.classList.add("hidden");
    $("pip-canvas")?.classList.add("hidden");
    $("pip-people")?.classList.remove("hidden");
  });

  listen("meeting://participants-update", (participants: PipParticipant[]) => {
    renderPeople(participants || []);
  });

  listen("meeting://left", () => window.close());

  $("pip-mic")?.addEventListener("click", async () => {
    const muted = await invoke<boolean>("toggle_meeting_mic");
    const btn = $("pip-mic");
    if (btn) {
      btn.textContent = muted ? "🔇" : "🎙️";
      btn.title = muted ? "Unmute" : "Mute";
    }
  });

  $("pip-share")?.addEventListener("click", async () => {
    // Sharing a new source needs the picker, so this only stops an active share.
    await invoke("toggle_meeting_screen_share", { source: null });
  });

  $("pip-open")?.addEventListener("click", () => invoke("focus_main_window"));

  $("pip-leave")?.addEventListener("click", async () => {
    await invoke("leave_lan_meeting");
    window.close();
  });

  // Fill in the current room without waiting for the next event.
  invoke<any>("get_meeting_status")
    .then((status) => {
      const room = $("pip-room");
      if (room && status?.room_id) room.textContent = `#${String(status.room_id).toUpperCase()}`;
      renderPeople(status?.participants || []);
      const btn = $("pip-mic");
      if (btn && status?.is_muted) btn.textContent = "🔇";
    })
    .catch(() => {});
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
