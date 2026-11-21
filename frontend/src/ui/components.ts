// Yeniden kullanılabilir UI ilkel öğeleri: DOM yardımcıları, simgeler, avatar,
// tost sunucusu, alt sayfa (bottom sheet), onay penceresi.

import { argbToCss } from "../types";
import { getToasts, subscribeToasts, type Toast } from "../state";

/** Öğe oluşturma yardımcısı. `attrs.class` ve olay dinleyicileri desteklenir. */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | boolean | ((e: Event) => void)> = {},
  children: (Node | string)[] = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (typeof v === "function") {
      node.addEventListener(k.startsWith("on") ? k.slice(2).toLowerCase() : k, v as EventListener);
    } else if (typeof v === "boolean") {
      if (v) node.setAttribute(k, "");
    } else {
      node.setAttribute(k, v);
    }
  }
  for (const c of children) node.append(c);
  return node;
}

/** Metin düğümü. */
export const txt = (s: string): Text => document.createTextNode(s);

/** SVG simge. `name` → inline SVG dizesi. */
export function icon(name: IconName, size = 22): HTMLElement {
  const span = el("span", { class: "icon", style: `width:${size}px;height:${size}px` });
  span.innerHTML = ICONS[name];
  return span;
}

export type IconName =
  | "send" | "stop" | "plus" | "gear" | "back" | "refresh" | "trash"
  | "menu" | "chevron" | "file" | "download" | "brain" | "check" | "close" | "key";

const ICONS: Record<IconName, string> = {
  send: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M3 20l18-8L3 4v6l13 2-13 2z"/></svg>`,
  stop: `<svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>`,
  plus: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 5v14M5 12h14"/></svg>`,
  gear: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H10a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V10a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>`,
  back: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>`,
  refresh: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.5 9a9 9 0 0 1 14.8-3.4L23 10M1 14l4.7 4.4A9 9 0 0 0 20.5 15"/></svg>`,
  trash: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m2 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>`,
  menu: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M3 12h18M3 18h18"/></svg>`,
  chevron: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 9l6 6 6-6"/></svg>`,
  file: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/></svg>`,
  download: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>`,
  brain: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 5a3 3 0 0 0-3 3v.5A2.5 2.5 0 0 0 7 13a2.5 2.5 0 0 0 2 4 2.5 2.5 0 0 0 3 1 2.5 2.5 0 0 0 3-1 2.5 2.5 0 0 0 2-4 2.5 2.5 0 0 0-2-4.5V8a3 3 0 0 0-3-3z"/></svg>`,
  check: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 6L9 17l-5-5"/></svg>`,
  close: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>`,
  key: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="7" cy="14" r="3"/><path d="M10 12l10-10M16 6l3 3"/></svg>`,
};

/** Hesap avatarı (renkli daire + baş harf). */
export function avatar(name: string, color: number, size = 36): HTMLElement {
  const node = el("div", { class: "avatar", style: `width:${size}px;height:${size}px;background:${argbToCss(color)};font-size:${Math.round(size * 0.42)}px` });
  node.textContent = (name.trim()[0] ?? "?").toUpperCase();
  return node;
}

/** Gan marka avatarı (mor daire + "G"). */
export function ganAvatar(size = 36): HTMLElement {
  const node = el("div", { class: "avatar gan-avatar", style: `width:${size}px;height:${size}px;font-size:${Math.round(size * 0.45)}px` });
  node.textContent = "G";
  return node;
}

/** Tost sunucusu (ekranın altında, kalıcı olmayan). */
export function mountToastHost(root: HTMLElement): void {
  const host = el("div", { class: "toast-host" });
  root.appendChild(host);
  const render = (items: Toast[]) => {
    host.replaceChildren(
      ...items.map((t) =>
        el("div", { class: `toast toast-${t.kind}` }, [txt(t.text)]),
      ),
    );
  };
  render(getToasts());
  subscribeToasts(render);
}

/** Alt sayfa (bottom sheet). `content` gövdesi. Dönüş: programatik kapatma fonksiyonu. */
export function bottomSheet(title: string, content: HTMLElement, onClose?: () => void): () => void {
  const overlay = el("div", { class: "sheet-overlay" });
  const sheet = el("div", { class: "bottom-sheet", onclick: (e) => e.stopPropagation() });
  const header = el("div", { class: "sheet-header" }, [
    el("h2", {}, [txt(title)]),
    el("button", { class: "sheet-close", onclick: () => close() }, [icon("close", 20)]),
  ]);
  const body = el("div", { class: "sheet-body" }, [content]);
  sheet.append(header, body);
  overlay.append(sheet);
  overlay.addEventListener("click", () => close());
  document.body.append(overlay);
  requestAnimationFrame(() => overlay.classList.add("open"));

  function close(): void {
    overlay.classList.remove("open");
    setTimeout(() => {
      overlay.remove();
      onClose?.();
    }, 220);
  }
  return close;
}

/** Onay penceresi. `onConfirm` onayda çağrılır. */
export function confirmDialog(title: string, message: string, onConfirm: () => void): void {
  const overlay = el("div", { class: "dialog-overlay" });
  const card = el("div", { class: "dialog" });
  card.append(
    el("h3", {}, [txt(title)]),
    el("p", { class: "dialog-msg" }, [txt(message)]),
    el("div", { class: "dialog-actions" }, [
      el("button", { class: "btn btn-text", onclick: () => close() }, [txt("Vazgeç")]),
      el("button", { class: "btn btn-danger", onclick: () => { close(); onConfirm(); } }, [txt("Sil")]),
    ]),
  );
  overlay.append(card);
  overlay.addEventListener("click", (e) => { if (e.target === overlay) close(); });
  document.body.append(overlay);
  requestAnimationFrame(() => overlay.classList.add("open"));
  function close(): void {
    overlay.classList.remove("open");
    setTimeout(() => overlay.remove(), 180);
  }
}

/** Üç noktalı "düşünüyor" animasyonu. */
export function thinkingDots(): HTMLElement {
  return el("div", { class: "thinking-dots" }, [
    el("span"), el("span"), el("span"),
  ]);
}
