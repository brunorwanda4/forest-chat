import type { AttachmentItem } from "../../types";
import { $, h, formatBytes } from "../../utils/dom";

export const AttachmentPreview = {
  render(attachments: AttachmentItem[], onRemove: (id: string) => void): void {
    const container = $("composer-attachments");
    if (!container) return;
    if (!attachments || attachments.length === 0) {
      container.replaceChildren();
      return;
    }

    const nodes = attachments.map((att) => {
      const isImg = att.mime.startsWith("image/") && att.previewUrl;
      return h(
        "div",
        {
          class:
            "group relative flex items-center gap-2 p-1.5 pr-2.5 bg-base-300/80 hover:bg-base-300 rounded-xl border border-base-content/10 shadow-xs transition-all max-w-[240px]",
          title: `${att.name} (${formatBytes(att.size)})`,
        },
        isImg
          ? h("img", {
              src: att.previewUrl!,
              alt: att.name,
              class: "w-10 h-10 object-cover rounded-lg border border-base-content/10 shrink-0",
            })
          : h(
              "div",
              {
                class:
                  "w-10 h-10 rounded-lg bg-primary/10 text-primary flex items-center justify-center shrink-0 text-lg font-bold",
              },
              "📄"
            ),
        h(
          "div",
          { class: "min-w-0 flex-1" },
          h("div", { class: "text-xs font-medium truncate leading-tight" }, att.name),
          h("div", { class: "text-[10px] text-base-content/60" }, formatBytes(att.size))
        ),
        h(
          "button",
          {
            type: "button",
            class:
              "btn btn-circle btn-ghost btn-xs text-base-content/60 hover:text-error hover:bg-base-100/50 -mr-1",
            title: "Remove file",
            "aria-label": `Remove ${att.name}`,
            onclick: (e: MouseEvent) => {
              e.preventDefault();
              e.stopPropagation();
              onRemove(att.id);
            },
          },
          "✕"
        )
      );
    });

    container.replaceChildren(...nodes);
  },
};
