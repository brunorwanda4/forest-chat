import { h } from "./utils/dom";

let openDmHandler: ((username: string) => void) | null = null;

export function setOpenDmHandler(handler: (username: string) => void): void {
  openDmHandler = handler;
}

export function renderRichText(text: string): HTMLElement {
  const container = h("div", { class: "rich-text-content leading-relaxed" });
  if (!text) return container;

  // Match triple backtick code blocks: ```lang?\ncode\n```
  const codeBlockRegex = /```([a-zA-Z0-9_-]*)\n([\s\S]*?)```|```([\s\S]*?)```/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = codeBlockRegex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      container.append(...renderTextAndSpans(text.slice(lastIndex, match.index)));
    }
    const codeContent = match[2] !== undefined ? match[2] : match[3];
    container.append(
      h(
        "pre",
        {
          class:
            "my-1.5 overflow-x-auto rounded-xl bg-base-300/80 p-2.5 font-mono text-xs border border-base-content/10 shadow-xs",
        },
        h("code", {}, codeContent)
      )
    );
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < text.length) {
    container.append(...renderTextAndSpans(text.slice(lastIndex)));
  }

  return container;
}

function renderTextAndSpans(segment: string): Node[] {
  const nodes: Node[] = [];
  const lines = segment.split("\n");

  let inBulletList = false;
  let bulletListEl: HTMLElement | null = null;
  let inNumList = false;
  let numListEl: HTMLElement | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Check bullet list: "- " or "* "
    const bMatch = line.match(/^(\s*)[-*]\s+(.+)$/);
    if (bMatch) {
      if (!inBulletList) {
        inBulletList = true;
        bulletListEl = h("ul", { class: "list-disc list-inside space-y-0.5 my-1 pl-1" });
        nodes.push(bulletListEl);
      }
      bulletListEl?.append(h("li", { class: "text-sm" }, ...renderInlineTokens(bMatch[2])));
      continue;
    } else {
      inBulletList = false;
      bulletListEl = null;
    }

    // Check numbered list: "1. "
    const nMatch = line.match(/^(\s*)\d+\.\s+(.+)$/);
    if (nMatch) {
      if (!inNumList) {
        inNumList = true;
        numListEl = h("ol", { class: "list-decimal list-inside space-y-0.5 my-1 pl-1" });
        nodes.push(numListEl);
      }
      numListEl?.append(h("li", { class: "text-sm" }, ...renderInlineTokens(nMatch[2])));
      continue;
    } else {
      inNumList = false;
      numListEl = null;
    }

    // Normal line
    const lineSpan = h("span", {}, ...renderInlineTokens(line));
    nodes.push(lineSpan);
    if (i < lines.length - 1) {
      nodes.push(document.createElement("br"));
    }
  }

  return nodes;
}

function renderInlineTokens(text: string): Node[] {
  const nodes: Node[] = [];
  // Match @mentions, `inline code`, **bold**, *italic*
  const tokenRegex = /(@[a-zA-Z0-9_-]{1,24})|(`[^`\n]+`)|(\*\*[^*]+\*\*)|(\*[^*]+\*)|(_[^_]+_)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenRegex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(document.createTextNode(text.slice(lastIndex, match.index)));
    }

    const full = match[0];
    if (match[1]) {
      // @mention pill
      const username = match[1].slice(1);
      const pill = h(
        "span",
        {
          class: "mention-pill",
          title: `Mentioned @${username} (click to message)`,
          onclick: (e: MouseEvent) => {
            e.stopPropagation();
            if (openDmHandler) {
              openDmHandler(username);
            }
          },
        },
        `@${username}`
      );
      nodes.push(pill);
    } else if (match[2]) {
      // `code`
      nodes.push(h("code", {}, full.slice(1, -1)));
    } else if (match[3]) {
      // **bold**
      nodes.push(h("strong", { class: "font-bold" }, full.slice(2, -2)));
    } else if (match[4]) {
      // *italic*
      nodes.push(h("em", { class: "italic" }, full.slice(1, -1)));
    } else if (match[5]) {
      // _italic_
      nodes.push(h("em", { class: "italic" }, full.slice(1, -1)));
    }

    lastIndex = match.index + full.length;
  }

  if (lastIndex < text.length) {
    nodes.push(document.createTextNode(text.slice(lastIndex)));
  }

  return nodes;
}
