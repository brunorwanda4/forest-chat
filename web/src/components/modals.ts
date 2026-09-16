import { state, send } from "../state";
import { $, h, avatar, avatarUrl, toast } from "../utils/dom";
import { timeFmt, ROOM_RE } from "../utils/format";

let renderRoomsCallback: (() => void) | null = null;

export function setRenderRoomsCallback(cb: () => void): void {
  renderRoomsCallback = cb;
}

export function openRequestJoinModal(room: string): void {
  state.pendingJoinRoom = room;
  const title = $("modal-request-join-title");
  const desc = $("modal-request-join-desc");
  const modal = $("modal-request-join") as HTMLDialogElement | null;

  if (title) title.textContent = `Request to Join #${room}`;
  if (desc) {
    desc.textContent = `#${room} requires admin approval. Click below to send a request to join. You will be able to view messages and chat once an admin approves your request.`;
  }
  modal?.showModal();
}

export function openManageGroupModal(room: string): void {
  const title = $("manage-group-title");
  const modal = $("modal-manage-group") as HTMLDialogElement | null;
  if (title) title.textContent = `Manage #${room}`;
  send({ type: "get_room_details", room });
  modal?.showModal();
}

export function renderManageGroupModal(): void {
  const details = state.activeRoomDetails;
  if (!details) return;

  const reqCountBadge = $("badge-requests-count");
  if (reqCountBadge) {
    if (details.requests && details.requests.length > 0) {
      reqCountBadge.textContent = String(details.requests.length);
      reqCountBadge.classList.remove("hidden");
    } else {
      reqCountBadge.classList.add("hidden");
    }
  }

  // Render requests
  const reqList = $("list-manage-requests");
  if (reqList) {
    if (!details.requests || details.requests.length === 0) {
      reqList.replaceChildren(
        h(
          "div",
          { class: "py-6 text-center text-xs text-base-content/50" },
          "No pending join requests."
        )
      );
    } else {
      const rows = details.requests.map((req) => {
        const timeStr = timeFmt.format(new Date(req.created_ts));
        return h(
          "div",
          { class: "flex items-center gap-3 p-2 bg-base-200 rounded-lg justify-between" },
          h(
            "div",
            { class: "flex items-center gap-2 min-w-0" },
            avatar(avatarUrl(req.username), "w-8"),
            h(
              "div",
              { class: "min-w-0" },
              h("div", { class: "font-semibold text-xs truncate" }, `@${req.username}`),
              h("div", { class: "text-[10px] text-base-content/60" }, `Requested at ${timeStr}`)
            )
          ),
          h(
            "div",
            { class: "flex items-center gap-1.5" },
            h(
              "button",
              {
                type: "button",
                class: "btn btn-xs btn-success text-white",
                onclick: () =>
                  send({ type: "approve_join", room: details.room, user: req.username }),
              },
              "Approve"
            ),
            h(
              "button",
              {
                type: "button",
                class: "btn btn-xs btn-ghost text-error hover:bg-error/20",
                onclick: () =>
                  send({ type: "reject_join", room: details.room, user: req.username }),
              },
              "Decline"
            )
          )
        );
      });
      reqList.replaceChildren(...rows);
    }
  }

  // Render members
  const memberList = $("list-manage-members");
  if (memberList) {
    if (!details.members || details.members.length === 0) {
      memberList.replaceChildren(
        h(
          "div",
          { class: "py-6 text-center text-xs text-base-content/50" },
          "No members found."
        )
      );
    } else {
      const rows = details.members.map((m) => {
        const isCreator = m.role === "creator";
        const isAdmin = m.role === "admin" || isCreator;
        const isMe = m.username === state.me;

        return h(
          "div",
          { class: "flex items-center gap-3 p-2 bg-base-200 rounded-lg justify-between" },
          h(
            "div",
            { class: "flex items-center gap-2 min-w-0" },
            avatar(avatarUrl(m.username), "w-8"),
            h(
              "div",
              { class: "min-w-0" },
              h(
                "div",
                { class: "font-semibold text-xs truncate flex items-center gap-1.5" },
                `@${m.username}`,
                isMe ? h("span", { class: "badge badge-xs badge-ghost" }, "you") : null
              ),
              h(
                "div",
                { class: "text-[10px] text-base-content/60" },
                isCreator ? "Group Creator" : isAdmin ? "Admin" : "Member"
              )
            )
          ),
          h(
            "div",
            { class: "flex items-center gap-2" },
            isCreator
              ? h(
                  "span",
                  { class: "badge badge-xs badge-warning font-bold gap-1" },
                  "👑 CREATOR"
                )
              : isAdmin
              ? h(
                  "span",
                  { class: "badge badge-xs badge-primary font-bold gap-1" },
                  "🛡️ ADMIN"
                )
              : h(
                  "button",
                  {
                    type: "button",
                    class: "btn btn-xs btn-outline btn-primary gap-1",
                    title: "Promote member to group admin",
                    onclick: () =>
                      send({ type: "promote_admin", room: details.room, user: m.username }),
                  },
                  "+ Make Admin"
                )
          )
        );
      });
      memberList.replaceChildren(...rows);
    }
  }
}

export function initGroupModals(): void {
  $("btn-submit-request-join")?.addEventListener("click", () => {
    if (!state.pendingJoinRoom) return;
    send({ type: "request_join", room: state.pendingJoinRoom });
    toast(`Join request sent for #${state.pendingJoinRoom}!`, "info");
    state.pending.add(state.pendingJoinRoom);
    if (renderRoomsCallback) renderRoomsCallback();
    ($("modal-request-join") as HTMLDialogElement | null)?.close();
  });

  $("btn-open-create-group")?.addEventListener("click", () => {
    const input = $("input-create-group-name") as HTMLInputElement | null;
    if (input) input.value = "";
    ($("modal-create-group") as HTMLDialogElement | null)?.showModal();
    input?.focus();
  });

  $("form-create-group")?.addEventListener("submit", (e) => {
    e.preventDefault();
    const input = $("input-create-group-name") as HTMLInputElement | null;
    const room = (input?.value || "").trim();
    if (!ROOM_RE.test(room)) return toast("Group names: 1–32 letters, numbers, - or _.");
    send({ type: "create_room", room });
    ($("modal-create-group") as HTMLDialogElement | null)?.close();
    toast(`Creating group #${room}…`, "info");
  });

  $("tab-manage-requests")?.addEventListener("click", () => {
    const tabReq = $("tab-manage-requests");
    const tabMem = $("tab-manage-members");
    if (tabReq) tabReq.className = "tab tab-active font-semibold text-xs sm:text-sm";
    if (tabMem) tabMem.className = "tab font-semibold text-xs sm:text-sm";
    $("panel-manage-requests")?.classList.remove("hidden");
    $("panel-manage-members")?.classList.add("hidden");
  });

  $("tab-manage-members")?.addEventListener("click", () => {
    const tabMem = $("tab-manage-members");
    const tabReq = $("tab-manage-requests");
    if (tabMem) tabMem.className = "tab tab-active font-semibold text-xs sm:text-sm";
    if (tabReq) tabReq.className = "tab font-semibold text-xs sm:text-sm";
    $("panel-manage-members")?.classList.remove("hidden");
    $("panel-manage-requests")?.classList.add("hidden");
  });
}
