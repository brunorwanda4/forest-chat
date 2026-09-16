export const $ = <T extends HTMLElement = HTMLElement>(id: string): T | null =>
  document.getElementById(id) as T | null;

export type Props = Record<string, any>;
export type Child = Node | string | number | boolean | null | undefined;

const SVG_TAGS = new Set([
  "svg",
  "path",
  "circle",
  "rect",
  "line",
  "polyline",
  "polygon",
  "g",
  "defs",
  "clipPath",
  "use",
]);

/** Safe DOM element constructor. Never uses innerHTML with user content. */
export function h<T extends HTMLElement | SVGElement = HTMLElement>(
  tag: string,
  props: Props = {},
  ...children: any[]
): T {
  const el = (
    SVG_TAGS.has(tag)
      ? document.createElementNS("http://www.w3.org/2000/svg", tag)
      : document.createElement(tag)
  ) as T;

  for (const [key, value] of Object.entries(props || {})) {
    if (value == null || value === false) continue;
    if (key.startsWith("on") && typeof value === "function") {
      el.addEventListener(key.slice(2).toLowerCase(), value as EventListener);
    } else if (key === "class") {
      el.setAttribute("class", String(value));
    } else if (key === "disabled") {
      if (value) el.setAttribute("disabled", "");
      else el.removeAttribute("disabled");
    } else {
      el.setAttribute(key, String(value));
    }
  }

  const flatten = (items: any[]) => {
    for (const child of items) {
      if (Array.isArray(child)) {
        flatten(child);
      } else if (child != null && child !== false) {
        el.append(child instanceof Node ? child : document.createTextNode(String(child)));
      }
    }
  };

  flatten(children);

  return el;
}

export const avatarUrl = (name: string): string =>
  `https://api.dicebear.com/9.x/adventurer/svg?seed=${encodeURIComponent(name)}`;

export const roomAvatarUrl = (room: string): string =>
  `https://api.dicebear.com/9.x/shapes/svg?seed=${encodeURIComponent(room)}`;

export function avatar(src: string, size = "w-10", online: boolean | null = null): HTMLElement {
  const cls =
    online == null
      ? "avatar"
      : online
      ? "avatar avatar-online"
      : "avatar avatar-offline";
  return h(
    "div",
    { class: cls },
    h("div", { class: `${size} rounded-full bg-base-300` }, h("img", { src, alt: "" }))
  );
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function toast(
  message: string,
  kind: "info" | "success" | "warning" | "error" = "error"
): void {
  const container = $("toast");
  if (!container) return;
  const el = h(
    "div",
    { class: `alert alert-${kind} shadow-lg text-sm py-2 px-3 flex items-center gap-2` },
    h("span", {}, message)
  );
  container.append(el);
  setTimeout(() => el.remove(), 3500);
}
