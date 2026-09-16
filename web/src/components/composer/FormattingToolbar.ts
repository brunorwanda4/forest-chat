import { $ } from "../../utils/dom";

export const FormattingToolbar = {
  active: false,

  init(textarea: HTMLTextAreaElement, onApply: (format: string) => void): void {
    const toolbar = $("composer-formatting-toolbar");
    if (!toolbar) return;

    toolbar.querySelectorAll<HTMLButtonElement>("[data-format]").forEach((btn) => {
      btn.addEventListener("mousedown", (e) => {
        e.preventDefault(); // keep textarea selection intact
        const format = btn.getAttribute("data-format");
        if (format) onApply(format);
      });
    });

    const updateVisibility = () => {
      if (document.activeElement !== textarea) {
        FormattingToolbar.hide();
        return;
      }
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      if (start !== end && textarea.value.slice(start, end).trim().length > 0) {
        FormattingToolbar.show();
      } else {
        FormattingToolbar.hide();
      }
    };

    textarea.addEventListener("select", updateVisibility);
    textarea.addEventListener("keyup", updateVisibility);
    textarea.addEventListener("mouseup", updateVisibility);
    textarea.addEventListener("blur", () => {
      setTimeout(() => {
        if (!toolbar.contains(document.activeElement)) {
          FormattingToolbar.hide();
        }
      }, 150);
    });
  },

  show(): void {
    const toolbar = $("composer-formatting-toolbar");
    if (!toolbar) return;
    toolbar.classList.remove("hidden");
    this.active = true;
  },

  hide(): void {
    const toolbar = $("composer-formatting-toolbar");
    if (!toolbar) return;
    toolbar.classList.add("hidden");
    this.active = false;
  },
};
