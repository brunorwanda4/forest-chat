import type { ChatAttachment } from "../types";
import { state } from "../state";
import { $, h, formatBytes, toast } from "../utils/dom";

export const isImage = (att: ChatAttachment): boolean => att.mime.startsWith("image/");
export const isVideo = (att: ChatAttachment): boolean => att.mime.startsWith("video/");
export const isAudio = (att: ChatAttachment): boolean => att.mime.startsWith("audio/");
export const isPdf = (att: ChatAttachment): boolean =>
  att.mime === "application/pdf" || /\.pdf$/i.test(att.name);
export const canPreview = (att: ChatAttachment): boolean =>
  isImage(att) || isVideo(att) || isAudio(att) || isPdf(att);

export const isDesktop = (): boolean =>
  Boolean((window as any).__TAURI_INTERNALS__ || (window as any).__TAURI__);

export async function tauriInvoke<T = any>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const win = window as any;
  if (win.__TAURI__?.core?.invoke) {
    return await win.__TAURI__.core.invoke(cmd, args);
  }
  if (win.__TAURI_INTERNALS__?.invoke) {
    return await win.__TAURI_INTERNALS__.invoke(cmd, args);
  }
  throw new Error("Tauri IPC is only available in the Forest Chat desktop app.");
}

export function fileUrl(att: ChatAttachment, download = false): string {
  const q = new URLSearchParams({
    name: state.me ?? "",
    token: state.token ?? "",
  });
  if (download) q.set("download", "1");
  return `/files/${encodeURIComponent(att.id)}?${q}`;
}

export const absoluteFileUrl = (att: ChatAttachment, download = false): string =>
  new URL(fileUrl(att, download), location.href).href;

export function fileIcon(att: ChatAttachment): string {
  if (isImage(att)) return "🖼️";
  if (isVideo(att)) return "🎬";
  if (isAudio(att)) return "🎵";
  if (isPdf(att)) return "📕";
  if (/\.(zip|rar|7z|tar|gz)$/i.test(att.name)) return "🗜️";
  return "📄";
}

export async function saveAttachment(att: ChatAttachment): Promise<void> {
  if (isDesktop()) {
    try {
      const path = await tauriInvoke<string>("save_attachment_as", {
        url: absoluteFileUrl(att, true),
        fileName: att.name,
      });
      if (path) toast(`Saved to ${path}`, "success");
    } catch (err: any) {
      toast(String(err?.message ?? err));
    }
    return;
  }
  const link = h("a", { href: fileUrl(att, true), download: att.name });
  document.body.append(link);
  link.click();
  link.remove();
}

export async function openAttachment(att: ChatAttachment): Promise<void> {
  if (isDesktop()) {
    try {
      await tauriInvoke("open_attachment_externally", {
        url: absoluteFileUrl(att),
        fileName: att.name,
      });
    } catch (err: any) {
      toast(String(err?.message ?? err));
    }
    return;
  }
  window.open(fileUrl(att), "_blank", "noopener");
}

export function openViewer(att: ChatAttachment): void {
  const url = fileUrl(att);
  const title = $("viewer-title");
  const meta = $("viewer-meta");
  const openBtn = $("viewer-open");
  const saveBtn = $("viewer-save");
  const modal = $("modal-viewer") as HTMLDialogElement | null;

  if (title) title.textContent = att.name;
  if (meta) meta.textContent = `${att.mime} · ${formatBytes(att.size)}`;
  if (openBtn) {
    openBtn.textContent = isDesktop() ? "Open in app" : "Open in tab";
    openBtn.onclick = () => openAttachment(att);
  }
  if (saveBtn) {
    saveBtn.onclick = () => saveAttachment(att);
  }

  let body: HTMLElement;
  if (isImage(att)) {
    body = h("img", { src: url, alt: att.name, class: "max-h-[75vh] object-contain" });
  } else if (isVideo(att)) {
    body = h("video", {
      src: url,
      controls: "",
      autoplay: "",
      class: "max-h-[75vh] w-full bg-black",
    });
  } else if (isAudio(att)) {
    body = h("audio", { src: url, controls: "", class: "w-full p-8" });
  } else if (isPdf(att)) {
    body = h("iframe", { src: url, title: att.name, class: "h-[75vh] w-full bg-base-100" });
  } else {
    body = h(
      "div",
      { class: "p-8 text-center text-base-content/70" },
      h("div", { class: "text-5xl" }, fileIcon(att)),
      h("p", { class: "pt-3" }, "No preview for this file type. Use Open or Save.")
    );
  }

  $("viewer-body")?.replaceChildren(body);
  modal?.showModal();
}

export function initViewerModalEvents(): void {
  $("modal-viewer")?.addEventListener("close", () => {
    $("viewer-body")?.replaceChildren();
  });
}

export function attachmentActions(att: ChatAttachment): HTMLElement {
  return h(
    "div",
    { class: "mt-1 flex flex-wrap gap-1" },
    canPreview(att)
      ? h(
          "button",
          { type: "button", class: "btn btn-xs", onclick: () => openViewer(att) },
          "View"
        )
      : null,
    h(
      "button",
      {
        type: "button",
        class: "btn btn-xs",
        onclick: () => openAttachment(att),
      },
      isDesktop() ? "Open in app" : "Open"
    ),
    h(
      "button",
      {
        type: "button",
        class: "btn btn-xs btn-primary",
        onclick: () => saveAttachment(att),
      },
      "Save"
    )
  );
}

export function attachmentNode(att: ChatAttachment): HTMLElement {
  const url = fileUrl(att);
  let preview: HTMLElement;

  if (isImage(att)) {
    preview = h("img", {
      src: url,
      alt: att.name,
      loading: "lazy",
      class: "max-h-72 max-w-full cursor-zoom-in rounded-lg object-contain",
      onclick: () => openViewer(att),
    });
  } else if (isVideo(att)) {
    preview = h("video", {
      src: url,
      controls: "",
      preload: "metadata",
      class: "max-h-72 max-w-full rounded-lg bg-black",
    });
  } else if (isAudio(att)) {
    preview = h("audio", { src: url, controls: "", class: "w-64 max-w-full" });
  } else {
    preview = h(
      "div",
      {
        class: "flex items-center gap-3 rounded-lg bg-base-100/20 p-2 cursor-pointer",
        onclick: () => (isPdf(att) ? openViewer(att) : openAttachment(att)),
      },
      h("span", { class: "text-3xl" }, fileIcon(att)),
      h(
        "div",
        { class: "min-w-0" },
        h("div", { class: "truncate text-sm font-medium" }, att.name),
        h("div", { class: "text-xs opacity-70" }, formatBytes(att.size))
      )
    );
  }

  return h("div", { class: "space-y-1" }, preview, attachmentActions(att));
}
