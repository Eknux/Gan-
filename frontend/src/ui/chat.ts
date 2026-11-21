// Sohbet ekranı: başlık (hesap + model), mesaj listesi, giriş çubuğu.
//
// Akış sırasında verimlilik için: devam eden asistan mesajı ham metni
// gösterir (her delta'da bubble güncellenir, markdown yeniden parse edilmez).
// Akış bitince (done/cancelled/error) o bubble markdown ile yeniden render edilir.

import {
  accountModel, hasApiKey, isGenerating, listMessages,
  newChat, sendMessage, stopGeneration, supportedModels,
} from "../api";
import { renderMarkdown, highlightCodeBlocks } from "../markdown";
import { activeAccount as getActiveAccount, getState, setState, subscribe, toast } from "../state";
import type { Msg } from "../types";
import { avatar, confirmDialog, el, ganAvatar, icon, thinkingDots, txt } from "./components";
import { openAccountSheet, openAddAccountDialog } from "./accounts";
import { openModelSheet } from "./settings";

let rootEl: HTMLElement;
let listEl: HTMLElement;
let inputEl: HTMLTextAreaElement;
let inputBarEl: HTMLElement;
let subscribed = false; // renderChat her çağrıda tekrar abone olmasın.

/** Sohbet ekranını `root` içine kur. İlk render + durum aboneliği (tek sefer). */
export function renderChat(root: HTMLElement): void {
  rootEl = root;
  root.replaceChildren(
    el("div", { class: "chat-app" }, [
      buildHeader(),
      el("div", { class: "chat-body", id: "chat-body" }, []),
      buildInputBar(),
    ]),
  );
  listEl = root.querySelector("#chat-body")!;
  inputBarEl = root.querySelector("#input-bar")!;
  inputEl = inputBarEl.querySelector("textarea")!;
  renderMessages();
  if (!subscribed) {
    subscribe(renderMessages);
    subscribed = true;
  }
}

function buildHeader(): HTMLElement {
  return el("header", { class: "chat-header" }, [
    el("div", { class: "header-left" }, [
      ganAvatar(34),
      el("div", { class: "header-title" }, [txt("Gan")]),
    ]),
    el("div", { class: "header-right" }, [
      el("button", { class: "icon-btn", title: "Yeni sohbet", onclick: onNewChat }, [icon("plus", 20)]),
      el("button", { class: "icon-btn", title: "Hesaplar", onclick: () => openAccountSheet() }, [icon("menu", 20)]),
      el("button", { class: "icon-btn", title: "Ayarlar", onclick: () => setState({ route: "settings" }) }, [icon("gear", 20)]),
    ]),
  ]);
}

/** Giriş çubuğu: textarea + gönder/durdur düğmeleri. `generating` durumuna
 *  göre düğme görünür değişir (`updateInputBar` ile). */
function buildInputBar(): HTMLElement {
  const ta = el("textarea", {
    class: "chat-input",
    placeholder: "Mesaj yaz…",
    rows: "1",
    oninput: () => autoGrow(inputEl),
    onkeydown: (e) => onKeyDown(e as KeyboardEvent),
  }) as HTMLTextAreaElement;
  // inputEl'i burada atayamayız (henuz DOM'da değil); renderChat'te querySelector ile alınır.
  // Ancak olay dinleyicileri inputEl'e başvurduğu için onları render sonrasına bağlamak
  // daha temiz olurdu. Basitlik için dinleyicileri inline tutuyoruz; autoGrow
  // render sonrası çağrıldığında inputEl dolu olacak.
  void ta;
  const sendBtn = el("button", {
    class: "send-btn",
    title: "Gönder",
    onclick: () => void onSend(),
  }, [icon("send", 20)]);
  const stopBtn = el("button", {
    class: "stop-btn",
    title: "Durdur",
    onclick: onStop,
  }, [icon("stop", 20)]);
  return el("div", { class: "chat-input-bar", id: "input-bar" }, [
    el("div", { class: "input-row" }, [
      // textarea'yı doğrudan burada oluştur (dinleyiciler gerçek öğeye bağlı):
      buildTextarea(),
      sendBtn,
      stopBtn,
    ]),
  ]);
}

function buildTextarea(): HTMLTextAreaElement {
  const ta = document.createElement("textarea");
  ta.className = "chat-input";
  ta.placeholder = "Mesaj yaz…";
  ta.rows = 1;
  ta.addEventListener("input", () => autoGrow(ta));
  ta.addEventListener("keydown", (e) => onKeyDown(e));
  return ta;
}

/** Başlık altındaki hesap+model satırı (aktif hesap varsa). */
function buildAccountBar(): HTMLElement | null {
  const acc = getActiveAccount();
  if (!acc) return null;
  return el("div", { class: "account-bar" }, [
    el("button", { class: "account-pill", onclick: () => openAccountSheet() }, [
      avatar(acc.name, acc.color, 24),
      el("span", { class: "account-name" }, [txt(acc.name)]),
    ]),
    el("button", { class: "model-pill", onclick: () => openModelSheet() }, [
      el("span", {}, [txt(getState().selectedModel)]),
      icon("chevron", 16),
    ]),
  ]);
}

function autoGrow(ta: HTMLTextAreaElement): void {
  ta.style.height = "auto";
  ta.style.height = `${Math.min(ta.scrollHeight, 160)}px`;
}

function onKeyDown(e: KeyboardEvent): void {
  // Enter gönderir; Shift+Enter yeni satır (mobil klavyede Shift genelde yok,
  // bu yüzden mobilde kullanıcı çok satırlı için yapıştırma kullanabilir).
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    void onSend();
  }
}

async function onSend(): Promise<void> {
  const acc = getActiveAccount();
  if (!acc) {
    openAddAccountDialog();
    return;
  }
  const text = inputEl.value.trim();
  if (!text || getState().generating) return;
  if (!(await hasApiKey(acc.id))) {
    toast("Önce API anahtarını gir (Ayarlar → API anahtarı)", "error");
    setState({ route: "settings" });
    return;
  }
  inputEl.value = "";
  autoGrow(inputEl);
  try {
    await sendMessage(acc.id, text);
  } catch (e) {
    toast(`Gönderilemedi: ${e}`, "error");
  }
}

function onStop(): void {
  const acc = getActiveAccount();
  if (acc) void stopGeneration(acc.id);
}

async function onNewChat(): Promise<void> {
  const acc = getActiveAccount();
  if (!acc) return;
  if (getState().messages.length === 0) return;
  confirmDialog("Yeni sohbet", "Bu sohbet silinecek. Devam?", async () => {
    try {
      await newChat(acc.id);
      setState({ messages: await listMessages(acc.id) });
    } catch (e) {
      toast(`Sohbet temizlenemedi: ${e}`, "error");
    }
  });
}

// ---- Mesaj listesi ----

/** Mesaj listesini state'ten yeniden render et. */
export function renderMessages(): void {
  // Hesap çubuğunu güncelle.
  const existing = rootEl.querySelector(".account-bar");
  const bar = buildAccountBar();
  if (bar) {
    if (existing) existing.replaceWith(bar);
    else rootEl.querySelector(".chat-header")?.after(bar);
  } else if (existing) {
    existing.remove();
  }

  const msgs = getState().messages;
  listEl.replaceChildren();
  // Giriş çubuğu: üretiyorsa "durdur" düğmesi, değilse "gönder".
  inputBarEl?.classList.toggle("generating", getState().generating);
  inputBarEl?.classList.toggle("no-account", !getActiveAccount());
  if (msgs.length === 0) {
    listEl.append(emptyState());
    return;
  }
  for (const m of msgs) listEl.append(messageNode(m));
  // Kod bloklarını vurgula.
  highlightCodeBlocks(listEl);
  scrollToBottom();
}

function emptyState(): HTMLElement {
  const acc = getActiveAccount();
  const inner = acc
    ? [ganAvatar(72), el("p", { class: "empty-title" }, [txt("Ne sormak istersin?")])]
    : [
        ganAvatar(72),
        el("h2", { class: "empty-title" }, [txt("Gan'a hoş geldin")]),
        el("p", { class: "empty-sub" }, [txt("Başlamak için bir Z.ai hesabı ekle.")]),
        el("button", { class: "btn btn-primary", onclick: () => openAddAccountDialog() }, [
          icon("plus", 18), txt("Hesap ekle"),
        ]),
      ];
  return el("div", { class: "empty-state" }, inner);
}

function messageNode(m: Msg): HTMLElement {
  if (m.role === "user") return userNode(m);
  return assistantNode(m);
}

function userNode(m: Msg): HTMLElement {
  return el("div", { class: "msg-row user" }, [
    el("div", { class: "bubble user-bubble" }, [txt(m.text)]),
  ]);
}

function assistantNode(m: Msg): HTMLElement {
  const acc = getActiveAccount();
  const row = el("div", { class: "msg-row assistant", "data-msg-id": String(m.id) });
  const avatarEl = acc ? avatar(acc.name, acc.color, 28) : ganAvatar(28);
  const content = el("div", { class: "assistant-content" });

  // Düşünce kartı.
  const thinkingActive = !m.done && m.text.length === 0;
  if (m.thinking || thinkingActive) {
    content.append(thinkingCard(m.thinking, thinkingActive));
  }

  // Yanıt gövdesi.
  if (m.error) {
    content.append(el("div", { class: "bubble assistant-bubble error-bubble" }, [txt(`⚠ ${m.error}`)]));
  } else if (m.text) {
    const bubble = el("div", { class: "bubble assistant-bubble" });
    if (m.done) {
      bubble.innerHTML = renderMarkdown(m.text);
    } else {
      // Akış sırasında: ham metin (escaped), her delta güncellenir.
      bubble.append(el("span", { class: "streaming-text" }, [txt(m.text)]));
    }
    content.append(bubble);
  } else if (m.done && m.files.length === 0) {
    content.append(el("div", { class: "bubble assistant-bubble empty-response" }, [txt("(Boş yanıt)")]));
  }

  // Dosya kartları.
  for (const f of m.files) {
    content.append(fileCard(f));
  }

  row.append(avatarEl, content);
  return row;
}

function thinkingCard(thinking: string, active: boolean): HTMLElement {
  const card = el("div", { class: "thinking-card" });
  const header = el("div", { class: "thinking-header" }, [
    icon("brain", 16),
    el("span", {}, [txt(active ? "Düşünüyor" : "Düşünce süreci")]),
    active ? thinkingDots() : icon("chevron", 16),
  ]);
  card.append(header);
  if (thinking) {
    const body = el("div", { class: "thinking-body" }, [txt(thinking)]);
    card.append(body);
    card.classList.toggle("open", !active);
    header.addEventListener("click", () => {
      card.classList.toggle("open");
      body.classList.toggle("hidden", !card.classList.contains("open"));
    });
    if (!card.classList.contains("open")) body.classList.add("hidden");
  }
  return card;
}

function fileCard(f: { name: string; url: string }): HTMLElement {
  return el("div", { class: "file-card" }, [
    icon("file", 20),
    el("span", { class: "file-name" }, [txt(f.name)]),
    el("a", { class: "btn btn-tonal file-link", href: f.url, target: "_blank", rel: "noopener noreferrer" }, [
      icon("download", 16), txt("İndir"),
    ]),
  ]);
}

// ---- Akış güncellemeleri (verimli: yalnızca bubble) ----

/**
 * `started`: yeni bir asistan mesajı eklendi; listeye tek düğüm ekle.
 * `delta`: streaming bubble'ın ham metnini güncelle (yeniden render yok).
 * `done`/`cancelled`/`error`: ilgili mesajı tam markdown ile yeniden render et.
 */
export function onStreamStarted(): void {
  // state.messages zaten main.ts tarafından güncellendi; son mesajı ekle.
  const msgs = getState().messages;
  const last = msgs[msgs.length - 1];
  if (!last || last.role !== "assistant") return;
  if (getState().messages.length === 1) {
    // ilk mesaj: boş durumu kaldır.
    listEl.replaceChildren();
  }
  listEl.append(messageNode(last));
  scrollToBottom();
}

export function onStreamDelta(msgId: number): void {
  const m = getState().messages.find((x) => x.id === msgId);
  if (!m) return;
  const row = listEl.querySelector<HTMLElement>(`[data-msg-id="${msgId}"]`);
  if (!row) return;
  // Ham metni güncelle.
  let span = row.querySelector<HTMLElement>(".streaming-text");
  let bubble = row.querySelector<HTMLElement>(".assistant-bubble");
  if (!bubble) {
    bubble = el("div", { class: "bubble assistant-bubble" });
    span = el("span", { class: "streaming-text" });
    bubble.append(span);
    row.querySelector(".assistant-content")?.append(bubble);
  }
  if (!span) {
    span = el("span", { class: "streaming-text" });
    bubble.replaceChildren(span);
  }
  span.textContent = m.text;
  // Düşünce kartını güncelle (varsa).
  let tc = row.querySelector<HTMLElement>(".thinking-card");
  if (m.thinking) {
    if (!tc) {
      tc = thinkingCard(m.thinking, !m.done && m.text.length === 0);
      row.querySelector(".assistant-content")?.prepend(tc);
    } else {
      const body = tc.querySelector(".thinking-body");
      if (body) body.textContent = m.thinking;
    }
  }
  scrollToBottom();
}

export function onStreamFinalize(msgId: number): void {
  const m = getState().messages.find((x) => x.id === msgId);
  if (!m) return;
  const old = listEl.querySelector(`[data-msg-id="${msgId}"]`);
  const fresh = messageNode(m);
  if (old) old.replaceWith(fresh);
  else listEl.append(fresh);
  highlightCodeBlocks(listEl);
  scrollToBottom();
}

function scrollToBottom(): void {
  const body = rootEl.querySelector("#chat-body");
  if (body) body.scrollTop = body.scrollHeight;
}

// İlk hesap/model yüklemesi (main.ts tarafından çağrılır).
export async function refreshActiveModel(): Promise<void> {
  const acc = getActiveAccount();
  if (!acc) return;
  try {
    const model = await accountModel(acc.id);
    setState({ selectedModel: model });
  } catch {
    /* yok say */
  }
  try {
    const gen = await isGenerating(acc.id);
    setState({ generating: gen });
  } catch {
    /* yok say */
  }
  void supportedModels().then((m) => setState({ supportedModels: m })).catch(() => {});
}
