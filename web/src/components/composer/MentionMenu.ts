import { state } from "../../state";
import { $, h, avatar, avatarUrl } from "../../utils/dom";

export interface MentionMember {
  username: string;
  role: string;
  isOnline: boolean;
  avatarUrl: string;
}

export const MentionMenu = {
  isOpen: false,
  items: [] as MentionMember[],
  selectedIndex: 0,
  query: "",
  mentionStart: -1,
  textarea: null as HTMLTextAreaElement | null,
  onSelect: null as ((member: MentionMember) => void) | null,

  init(textarea: HTMLTextAreaElement, onSelect: (member: MentionMember) => void): void {
    this.textarea = textarea;
    this.onSelect = onSelect;

    textarea.addEventListener("input", () => this.checkTrigger());
    textarea.addEventListener("keydown", (e) => this.handleKey(e));
    textarea.addEventListener("click", () => this.checkTrigger());

    document.addEventListener("click", (e) => {
      if (
        this.isOpen &&
        !$("composer-mention-menu")?.contains(e.target as Node) &&
        e.target !== textarea
      ) {
        this.close();
      }
    });
  },

  getAvailableMembers(): MentionMember[] {
    const chat = state.active;
    if (!chat) return [];

    const membersMap = new Map<string, MentionMember>();

    if (chat.kind === "room") {
      if (state.activeRoomDetails?.room === chat.id && state.activeRoomDetails.members) {
        for (const m of state.activeRoomDetails.members) {
          membersMap.set(m.username, {
            username: m.username,
            role: m.role,
            isOnline: state.users.includes(m.username),
            avatarUrl: avatarUrl(m.username),
          });
        }
      }
      for (const u of state.users) {
        if (!membersMap.has(u)) {
          membersMap.set(u, {
            username: u,
            role: "member",
            isOnline: true,
            avatarUrl: avatarUrl(u),
          });
        }
      }
    } else if (chat.kind === "dm") {
      membersMap.set(chat.id, {
        username: chat.id,
        role: "dm",
        isOnline: state.users.includes(chat.id),
        avatarUrl: avatarUrl(chat.id),
      });
      if (state.me) {
        membersMap.set(state.me, {
          username: state.me,
          role: "you",
          isOnline: true,
          avatarUrl: avatarUrl(state.me),
        });
      }
    }

    return Array.from(membersMap.values()).sort((a, b) => {
      if (a.isOnline !== b.isOnline) return a.isOnline ? -1 : 1;
      return a.username.localeCompare(b.username);
    });
  },

  checkTrigger(): void {
    if (!this.textarea) return;
    const cursorPos = this.textarea.selectionStart;
    const textBefore = this.textarea.value.slice(0, cursorPos);
    const match = textBefore.match(/(?:^|\s)@([a-zA-Z0-9_-]*)$/);

    if (match) {
      this.query = match[1].toLowerCase();
      this.mentionStart = cursorPos - match[1].length - 1;
      const allMembers = this.getAvailableMembers();
      this.items = allMembers.filter((m) =>
        m.username.toLowerCase().includes(this.query)
      );

      if (this.items.length > 0) {
        this.selectedIndex = Math.min(this.selectedIndex, this.items.length - 1);
        this.open();
        this.render();
      } else {
        this.close();
      }
    } else {
      this.close();
    }
  },

  handleKey(e: KeyboardEvent): boolean {
    if (!this.isOpen || this.items.length === 0) return false;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      this.selectedIndex = (this.selectedIndex + 1) % this.items.length;
      this.render();
      this.scrollSelectedIntoView();
      return true;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      this.selectedIndex = (this.selectedIndex - 1 + this.items.length) % this.items.length;
      this.render();
      this.scrollSelectedIntoView();
      return true;
    }
    if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      const selected = this.items[this.selectedIndex];
      if (selected) this.selectMember(selected);
      return true;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      this.close();
      return true;
    }
    return false;
  },

  scrollSelectedIntoView(): void {
    const list = $("mention-menu-list");
    const active = list?.children[this.selectedIndex] as HTMLElement | undefined;
    if (active) active.scrollIntoView({ block: "nearest" });
  },

  selectMember(member: MentionMember): void {
    if (!this.textarea) return;
    const val = this.textarea.value;
    const before = val.slice(0, this.mentionStart);
    const after = val.slice(this.textarea.selectionStart);
    const insertion = `@${member.username} `;
    this.textarea.value = before + insertion + after;
    const newPos = before.length + insertion.length;
    this.textarea.setSelectionRange(newPos, newPos);
    this.close();
    this.textarea.focus();
    if (this.onSelect) {
      this.onSelect(member);
    }
  },

  open(): void {
    this.isOpen = true;
    $("composer-mention-menu")?.classList.remove("hidden");
  },

  close(): void {
    this.isOpen = false;
    $("composer-mention-menu")?.classList.add("hidden");
  },

  render(): void {
    const list = $("mention-menu-list");
    if (!list) return;

    const rows = this.items.map((m, idx) => {
      const isSelected = idx === this.selectedIndex;
      const isCreator = m.role === "creator";
      const isAdmin = m.role === "admin";
      const isMe = m.username === state.me;

      return h(
        "div",
        {
          class: `flex items-center gap-2.5 px-2.5 py-1.5 rounded-xl cursor-pointer transition-colors ${
            isSelected
              ? "bg-primary text-primary-content font-medium shadow-xs"
              : "hover:bg-base-200 text-base-content"
          }`,
          role: "option",
          "aria-selected": isSelected ? "true" : "false",
          onmouseenter: () => {
            this.selectedIndex = idx;
            this.render();
          },
          onclick: (e: MouseEvent) => {
            e.preventDefault();
            this.selectMember(m);
          },
        },
        avatar(m.avatarUrl, "w-7", m.isOnline),
        h(
          "div",
          { class: "min-w-0 flex-1 flex items-center justify-between gap-1" },
          h(
            "div",
            { class: "flex items-center gap-1.5 truncate" },
            h("span", { class: "text-xs font-semibold truncate" }, `@${m.username}`),
            isMe
              ? h(
                  "span",
                  {
                    class: `badge badge-xs ${
                      isSelected
                        ? "badge-ghost bg-primary-content/20 text-primary-content"
                        : "badge-ghost"
                    }`,
                  },
                  "you"
                )
              : null
          ),
          h(
            "div",
            { class: "flex items-center gap-1 shrink-0" },
            isCreator
              ? h("span", { class: "badge badge-xs badge-warning font-bold scale-90" }, "👑")
              : isAdmin
              ? h("span", { class: "badge badge-xs badge-primary font-bold scale-90" }, "🛡️")
              : null,
            h("span", {
              class: `w-2 h-2 rounded-full ${
                m.isOnline ? "bg-success" : "bg-base-content/30"
              }`,
              title: m.isOnline ? "Online" : "Offline",
            })
          )
        )
      );
    });

    list.replaceChildren(...rows);
  },
};
