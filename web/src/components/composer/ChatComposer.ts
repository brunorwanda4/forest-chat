import type { ActiveChat, AttachmentItem } from "../../types";
import { state } from "../../state";
import { $, toast } from "../../utils/dom";
import { AttachmentPreview } from "./AttachmentPreview";
import { FormattingToolbar } from "./FormattingToolbar";
import { MentionMenu } from "./MentionMenu";
import { EmojiPicker } from "./EmojiPicker";

let uploadSeq = 0;

export interface DraftMessage {
  text: string;
  attachments: AttachmentItem[];
}

export type SendHandler = (params: {
  type: "send";
  chat: ActiveChat;
  text: string;
  attachment?: {
    id: string;
    name: string;
    size: number;
    mime: string;
  };
}) => void;

let sendCallback: SendHandler | null = null;
let startTypingCallback: (() => void) | null = null;
let stopTypingCallback: (() => void) | null = null;

export function setSendCallback(cb: SendHandler): void {
  sendCallback = cb;
}

export function setTypingCallbacks(onStart: () => void, onStop: () => void): void {
  startTypingCallback = onStart;
  stopTypingCallback = onStop;
}

export const ChatComposer = {
  draft: {
    text: "",
    attachments: [] as AttachmentItem[],
  },

  textarea: null as HTMLTextAreaElement | null,
  sendBtn: null as HTMLButtonElement | null,
  attachBtn: null as HTMLButtonElement | null,
  emojiBtn: null as HTMLButtonElement | null,
  charCount: null as HTMLElement | null,
  container: null as HTMLElement | null,
  dropzone: null as HTMLElement | null,

  init(): void {
    this.textarea = $("composer-input") as HTMLTextAreaElement | null;
    this.sendBtn = $("composer-send") as HTMLButtonElement | null;
    this.attachBtn = $("btn-attach") as HTMLButtonElement | null;
    this.emojiBtn = $("btn-emoji") as HTMLButtonElement | null;
    this.charCount = $("composer-char-count");
    this.container = $("composer-container");
    this.dropzone = $("composer-dropzone");

    if (!this.textarea) return;

    AttachmentPreview.render(this.draft.attachments, (id) => this.removeAttachment(id));

    FormattingToolbar.init(this.textarea, (fmt) => {
      this.applyFormat(fmt);
      this.onInput();
    });

    MentionMenu.init(this.textarea, () => {
      this.onInput();
    });

    EmojiPicker.init(this.textarea, () => {
      this.onInput();
    });

    this.textarea.addEventListener("input", () => this.onInput());
    this.textarea.addEventListener("keydown", (e) => this.onKeyDown(e));
    this.textarea.addEventListener("paste", (e) => this.onPaste(e));
    this.textarea.addEventListener("blur", () => {
      if (stopTypingCallback) stopTypingCallback();
    });

    $("composer")?.addEventListener("submit", (e) => {
      e.preventDefault();
      this.send();
    });

    this.attachBtn?.addEventListener("click", () => {
      ($("file-input") as HTMLInputElement | null)?.click();
    });

    $("file-input")?.addEventListener("change", (e) => {
      const target = e.target as HTMLInputElement;
      const files = target.files ? Array.from(target.files) : [];
      target.value = "";
      if (files.length > 0) this.addFiles(files);
    });

    this.setupDragAndDrop();
  },

  applyFormat(format: string): void {
    if (!this.textarea) return;
    const start = this.textarea.selectionStart;
    const end = this.textarea.selectionEnd;
    const text = this.textarea.value;
    const selected = text.slice(start, end);

    let prefix = "";
    let suffix = "";
    switch (format) {
      case "bold":
        prefix = "**";
        suffix = "**";
        break;
      case "italic":
        prefix = "*";
        suffix = "*";
        break;
      case "code":
        prefix = "`";
        suffix = "`";
        break;
      case "bullet":
        prefix = "- ";
        break;
      case "number":
        prefix = "1. ";
        break;
      case "codeblock":
        prefix = "```\n";
        suffix = "\n```";
        break;
    }

    const replacement = prefix + selected + suffix;
    this.textarea.value = text.slice(0, start) + replacement + text.slice(end);
    this.textarea.setSelectionRange(
      start + prefix.length,
      start + prefix.length + selected.length
    );
    this.textarea.focus();
  },

  setupDragAndDrop(): void {
    let dragCounter = 0;
    const dropzone = this.dropzone;
    if (!dropzone) return;

    window.addEventListener("dragenter", (e) => {
      if (!state.active) return;
      if (e.dataTransfer?.types?.includes("Files")) {
        dragCounter++;
        dropzone.classList.remove("hidden");
      }
    });

    window.addEventListener("dragleave", () => {
      if (!state.active) return;
      dragCounter = Math.max(0, dragCounter - 1);
      if (dragCounter === 0) dropzone.classList.add("hidden");
    });

    window.addEventListener("dragover", (e) => {
      if (!state.active) return;
      e.preventDefault();
    });

    window.addEventListener("drop", (e) => {
      if (!state.active) return;
      e.preventDefault();
      dragCounter = 0;
      dropzone.classList.add("hidden");
      if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
        this.addFiles(Array.from(e.dataTransfer.files));
      }
    });
  },

  onPaste(e: ClipboardEvent): void {
    if (!state.active) return;
    const clipboardFiles = e.clipboardData?.files;
    if (clipboardFiles && clipboardFiles.length > 0) {
      e.preventDefault();
      this.addFiles(Array.from(clipboardFiles));
    }
  },

  addFiles(files: File[]): void {
    for (const file of files) {
      const id = "att-" + ++uploadSeq;
      const isImg = file.type.startsWith("image/");
      const previewUrl = isImg ? URL.createObjectURL(file) : null;
      this.draft.attachments.push({
        id,
        file,
        name: file.name,
        size: file.size,
        mime: file.type || "application/octet-stream",
        previewUrl,
      });
    }
    AttachmentPreview.render(this.draft.attachments, (id) => this.removeAttachment(id));
    this.updateState();
    this.textarea?.focus();
  },

  removeAttachment(id: string): void {
    const idx = this.draft.attachments.findIndex((a) => a.id === id);
    if (idx !== -1) {
      const [removed] = this.draft.attachments.splice(idx, 1);
      if (removed.previewUrl) URL.revokeObjectURL(removed.previewUrl);
    }
    AttachmentPreview.render(this.draft.attachments, (id) => this.removeAttachment(id));
    this.updateState();
    this.textarea?.focus();
  },

  clearAttachments(): void {
    for (const att of this.draft.attachments) {
      if (att.previewUrl) URL.revokeObjectURL(att.previewUrl);
    }
    this.draft.attachments = [];
    AttachmentPreview.render(this.draft.attachments, (id) => this.removeAttachment(id));
  },

  onInput(): void {
    this.autoResize();
    this.updateState();

    if (this.textarea && this.textarea.value.trim()) {
      if (startTypingCallback) startTypingCallback();
    } else {
      if (stopTypingCallback) stopTypingCallback();
    }
  },

  autoResize(): void {
    if (!this.textarea) return;
    this.textarea.style.height = "auto";
    const height = Math.min(Math.max(this.textarea.scrollHeight, 42), 180);
    this.textarea.style.height = `${height}px`;
  },

  onKeyDown(e: KeyboardEvent): void {
    if (!this.textarea) return;

    if (MentionMenu.isOpen && MentionMenu.handleKey(e)) {
      return;
    }

    if (e.key === "Enter" && e.shiftKey) {
      setTimeout(() => this.autoResize(), 0);
      return;
    }

    if (e.key === "Enter" && !e.shiftKey) {
      const cursorPos = this.textarea.selectionStart;
      const text = this.textarea.value;
      const currentLineStart = text.lastIndexOf("\n", cursorPos - 1) + 1;
      const currentLine = text.slice(currentLineStart, cursorPos);

      // Check bullet list continuation: "- " or "* "
      const bulletMatch = currentLine.match(/^(\s*[-*]\s+)(.*)$/);
      if (bulletMatch) {
        e.preventDefault();
        const prefix = bulletMatch[1];
        const content = bulletMatch[2];
        if (!content.trim()) {
          this.textarea.value = text.slice(0, currentLineStart) + text.slice(cursorPos);
          this.textarea.setSelectionRange(currentLineStart, currentLineStart);
        } else {
          const ins = "\n" + prefix;
          this.textarea.value = text.slice(0, cursorPos) + ins + text.slice(cursorPos);
          const nextPos = cursorPos + ins.length;
          this.textarea.setSelectionRange(nextPos, nextPos);
        }
        this.onInput();
        return;
      }

      // Check numbered list continuation: "1. "
      const numMatch = currentLine.match(/^(\s*)(\d+)\.\s+(.*)$/);
      if (numMatch) {
        e.preventDefault();
        const indent = numMatch[1];
        const num = parseInt(numMatch[2], 10);
        const content = numMatch[3];
        if (!content.trim()) {
          this.textarea.value = text.slice(0, currentLineStart) + text.slice(cursorPos);
          this.textarea.setSelectionRange(currentLineStart, currentLineStart);
        } else {
          const ins = `\n${indent}${num + 1}. `;
          this.textarea.value = text.slice(0, cursorPos) + ins + text.slice(cursorPos);
          const nextPos = cursorPos + ins.length;
          this.textarea.setSelectionRange(nextPos, nextPos);
        }
        this.onInput();
        return;
      }

      // Plain enter: send message
      e.preventDefault();
      this.send();
    }
  },

  updateState(): void {
    if (!this.textarea) return;
    const text = this.textarea.value.trim();
    const hasContent = text.length > 0 || this.draft.attachments.length > 0;
    const isEnabled = Boolean(state.active && hasContent);
    if (this.sendBtn) this.sendBtn.disabled = !isEnabled;

    const len = this.textarea.value.length;
    if (this.charCount) {
      if (len >= 1200) {
        this.charCount.classList.remove("hidden");
        this.charCount.textContent = `${len.toLocaleString()} / 2,000`;
        if (len >= 1950) {
          this.charCount.className = "text-[11px] font-mono tabular-nums text-error font-bold";
        } else if (len >= 1600) {
          this.charCount.className = "text-[11px] font-mono tabular-nums text-warning font-medium";
        } else {
          this.charCount.className = "text-[11px] font-mono tabular-nums text-base-content/50";
        }
      } else {
        this.charCount.classList.add("hidden");
      }
    }
  },

  async send(): Promise<void> {
    if (!this.textarea || !state.active) return;
    const text = this.textarea.value.trim();
    const attachments = [...this.draft.attachments];
    if (!text && attachments.length === 0) return;

    if (stopTypingCallback) stopTypingCallback();
    if (this.sendBtn) this.sendBtn.disabled = true;

    if (attachments.length > 0) {
      const originalSendBtnHtml = this.sendBtn ? this.sendBtn.innerHTML : "";
      if (this.sendBtn) {
        this.sendBtn.innerHTML = '<span class="loading loading-spinner loading-xs"></span> Sending…';
      }

      for (let i = 0; i < attachments.length; i++) {
        const att = attachments[i];
        const isFirst = i === 0;
        try {
          await new Promise<void>((resolve, reject) => {
            const xhr = new XMLHttpRequest();
            const query = new URLSearchParams({
              name: state.me ?? "",
              token: state.token ?? "",
            });
            xhr.open("POST", `/api/upload?${query}`);
            const form = new FormData();
            form.append("file", att.file, att.name);

            xhr.addEventListener("load", () => {
              let body: any = null;
              try {
                body = JSON.parse(xhr.responseText);
              } catch (_) {}
              if (xhr.status === 200 && body?.id) {
                if (sendCallback) {
                  sendCallback({
                    type: "send",
                    chat: state.active!,
                    text: isFirst ? text : "",
                    attachment: body,
                  });
                }
                resolve();
              } else {
                toast(body?.error ?? `Upload failed for ${att.name}`);
                reject();
              }
            });
            xhr.addEventListener("error", () => {
              toast(`Could not send ${att.name}`);
              reject();
            });
            xhr.send(form);
          });
        } catch (_) {
          if (this.sendBtn) this.sendBtn.innerHTML = originalSendBtnHtml;
          this.updateState();
          return;
        }
      }
      if (this.sendBtn) this.sendBtn.innerHTML = originalSendBtnHtml;
    } else {
      if (sendCallback) {
        sendCallback({ type: "send", chat: state.active, text });
      }
    }

    this.textarea.value = "";
    this.clearAttachments();
    this.autoResize();
    this.updateState();
    this.textarea.focus();
  },

  setActiveChat(chat: ActiveChat | null): void {
    if (!this.textarea) return;
    if (chat) {
      this.textarea.disabled = false;
      if (this.attachBtn) this.attachBtn.disabled = false;
      if (this.emojiBtn) this.emojiBtn.disabled = false;
      this.textarea.placeholder =
        chat.kind === "room"
          ? chat.id === "general"
            ? "Message the group…"
            : `Message #${chat.id}…`
          : `Message @${chat.id}…`;
      this.updateState();
      this.textarea.focus();
    } else {
      this.textarea.disabled = true;
      if (this.attachBtn) this.attachBtn.disabled = true;
      if (this.emojiBtn) this.emojiBtn.disabled = true;
      if (this.sendBtn) this.sendBtn.disabled = true;
      this.textarea.placeholder = "Pick a chat to start";
      this.textarea.value = "";
      this.clearAttachments();
      this.updateState();
    }
  },
};
