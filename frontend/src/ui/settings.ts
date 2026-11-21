// Ayarlar ekranı + model seçici alt sayfa + API anahtarı penceresi.

import {
  accountModel, getSettings, hasApiKey, saveSettings, setAccountModel,
  setApiKey, supportedModels, testApiKey,
} from "../api";
import { activeAccount as getActiveAccount, getState, setState, toast } from "../state";
import type { Theme } from "../types";
import { bottomSheet, el, icon, txt } from "./components";

/** Ayarlar ekranını `root` içine render et. */
export async function renderSettings(root: HTMLElement): Promise<void> {
  const settings = await getSettings();
  setState({ settings });
  const acc = getActiveAccount();
  const keySet = acc ? await hasApiKey(acc.id) : false;

  const apiBox = el("div", { class: "setting-card" }, [
    el("div", { class: "setting-row" }, [
      icon("key", 20),
      el("div", { class: "setting-text" }, [
        el("div", { class: "setting-title" }, [txt("API anahtarı")]),
        el("div", { class: "setting-sub" }, [
          txt(acc ? (keySet ? "Tanımlı (şifreli)" : "Tanımlı değil") : "Önce hesap seç"),
        ]),
      ]),
    ]),
    el("div", { class: "setting-actions" }, [
      el("button", {
        class: "btn btn-tonal",
        onclick: () => acc && openApiKeyDialog(acc.id),
      }, [txt("Değiştir")]),
      el("button", {
        class: "btn btn-text",
        onclick: () => acc && void onTestKey(acc.id),
      }, [txt("Doğrula")]),
    ]),
  ]);

  root.replaceChildren(
    el("div", { class: "settings-app" }, [
      el("header", { class: "chat-header" }, [
        el("button", { class: "icon-btn", title: "Geri", onclick: () => setState({ route: "chat" }) }, [icon("back", 22)]),
        el("div", { class: "header-title" }, [txt("Ayarlar")]),
      ]),
      el("div", { class: "settings-body" }, [
        apiBox,

        sectionTitle("Model"),
        selectCard("Varsayılan model", settings.default_model, await supportedModels().catch(() => ["glm-5.2"]),
          (v) => { settings.default_model = v; markDirty(); }),
        toggleCard("Düşünme (chain-of-thought)", settings.thinking_enabled,
          (v) => { settings.thinking_enabled = v; markDirty(); }),
        selectCard("Düşünme çabası", settings.reasoning_effort, ["max", "xhigh", "high", "medium", "low", "minimal", "none"],
          (v) => { settings.reasoning_effort = v; markDirty(); }),
        rangeCard("Sıcaklık", settings.temperature, 0, 1, 0.05,
          (v) => { settings.temperature = v; markDirty(); }),

        sectionTitle("Sistem promptu"),
        textareaCard("Her yeni sohbetin başına eklenir (boş bırakılabilir).", settings.system_prompt,
          (v) => { settings.system_prompt = v; markDirty(); }),

        sectionTitle("Görünüm"),
        selectCard("Tema", settings.theme, ["system", "light", "dark"],
          (v) => { const t = v as Theme; settings.theme = t; applyTheme(t); markDirty(); }),

        sectionTitle("Gelişmiş"),
        textCard("API kök adresi", settings.api_url,
          (v) => { settings.api_url = v; markDirty(); }),

        el("button", { class: "btn btn-primary block save-btn", id: "save-btn", onclick: () => void onSave() }, [
          txt("Kaydet"),
        ]),
      ]),
    ]),
  );
  applyTheme(settings.theme);

  function markDirty(): void {
    const btn = root.querySelector("#save-btn");
    if (btn) btn.classList.add("dirty");
  }
  async function onSave(): Promise<void> {
    try {
      await saveSettings(settings);
      setState({ settings });
      toast("Ayarlar kaydedildi", "success");
      root.querySelector("#save-btn")?.classList.remove("dirty");
    } catch (e) {
      toast(`Kaydedilemedi: ${e}`, "error");
    }
  }
}

async function onTestKey(accountId: string): Promise<void> {
  toast("Doğrulanıyor…");
  try {
    await testApiKey(accountId);
    toast("API anahtarı geçerli", "success");
  } catch (e) {
    toast(`Doğrulama başarısız: ${e}`, "error");
  }
}

/** Model seçici alt sayfa (aktif hesabın modeli). */
export async function openModelSheet(): Promise<void> {
  const acc = getActiveAccount();
  if (!acc) return;
  const models = await supportedModels().catch(() => ["glm-5.2"]);
  let current = getState().selectedModel;
  try { current = await accountModel(acc.id); } catch { /* yok say */ }
  const content = el("div", { class: "sheet-list" });
  for (const m of models) {
    const row = el("div", { class: `model-row ${m === current ? "active" : ""}` }, [
      el("span", { class: "radio" }, [m === current ? icon("check", 16) : txt("")]),
      el("span", { class: "model-name" }, [txt(m)]),
    ]);
    row.addEventListener("click", async () => {
      try {
        await setAccountModel(acc.id, m);
        setState({ selectedModel: m });
        close();
      } catch (e) {
        toast(`Seçilemedi: ${e}`, "error");
      }
    });
    content.append(row);
  }
  const close = bottomSheet("Model", content);
}

/** API anahtarı değiştirme penceresi. */
function openApiKeyDialog(accountId: string): void {
  const overlay = el("div", { class: "dialog-overlay" });
  const card = el("div", { class: "dialog wide" });
  const input = el("input", {
    class: "text-field",
    type: "password",
    placeholder: "Yeni z.ai API anahtarı",
    autocomplete: "off",
    spellcheck: "false",
  });
  card.append(
    el("h3", {}, [txt("API anahtarını değiştir")]),
    el("p", { class: "dialog-sub" }, [txt("Eski anahtarın üzerine yazılır. Cihazda şifrelenir.")]),
    input,
    el("div", { class: "dialog-actions" }, [
      el("button", { class: "btn btn-text", onclick: close }, [txt("Vazgeç")]),
      el("button", { class: "btn btn-primary", onclick: () => void onSave() }, [txt("Kaydet")]),
    ]),
  );
  overlay.append(card);
  overlay.addEventListener("click", (e) => { if (e.target === overlay) close(); });
  document.body.append(overlay);
  requestAnimationFrame(() => overlay.classList.add("open"));
  input.focus();
  function close(): void {
    overlay.classList.remove("open");
    setTimeout(() => overlay.remove(), 180);
  }
  async function onSave(): Promise<void> {
    const key = input.value.trim();
    if (!key) { toast("Anahtar boş olamaz", "error"); return; }
    try {
      await setApiKey(accountId, key);
      close();
      toast("API anahtarı kaydedildi", "success");
      void renderSettings(document.getElementById("app")!);
    } catch (e) {
      toast(`Kaydedilemedi: ${e}`, "error");
    }
  }
}

// ---- Ayar kart yardımcıları ----

function sectionTitle(t: string): HTMLElement {
  return el("h2", { class: "section-title" }, [txt(t)]);
}

function selectCard(label: string, value: string, options: string[], onChange: (v: string) => void): HTMLElement {
  const sel = document.createElement("select");
  sel.className = "select-field";
  for (const o of options) {
    const opt = document.createElement("option");
    opt.value = o;
    opt.textContent = o;
    if (o === value) opt.selected = true;
    sel.append(opt);
  }
  sel.addEventListener("change", () => onChange(sel.value));
  return el("div", { class: "setting-card" }, [
    el("div", { class: "setting-row" }, [
      el("span", { class: "setting-label" }, [txt(label)]),
      sel,
    ]),
  ]);
}

function toggleCard(label: string, value: boolean, onChange: (v: boolean) => void): HTMLElement {
  const sw = el("button", { class: `switch ${value ? "on" : ""}`, role: "switch", "aria-checked": String(value) });
  sw.addEventListener("click", () => {
    const next = !sw.classList.contains("on");
    sw.classList.toggle("on", next);
    sw.setAttribute("aria-checked", String(next));
    onChange(next);
  });
  return el("div", { class: "setting-card" }, [
    el("div", { class: "setting-row" }, [
      el("span", { class: "setting-label" }, [txt(label)]),
      sw,
    ]),
  ]);
}

function rangeCard(label: string, value: number, min: number, max: number, step: number, onChange: (v: number) => void): HTMLElement {
  const range = el("input", { class: "range-field", type: "range", min: String(min), max: String(max), step: String(step), value: String(value) }) as HTMLInputElement;
  const valLabel = el("span", { class: "range-value" }, [txt(value.toFixed(2))]);
  range.addEventListener("input", () => {
    const v = parseFloat(range.value);
    valLabel.textContent = v.toFixed(2);
    onChange(v);
  });
  return el("div", { class: "setting-card" }, [
    el("div", { class: "setting-row" }, [
      el("span", { class: "setting-label" }, [txt(label)]),
      el("div", { class: "range-row" }, [range, valLabel]),
    ]),
  ]);
}

function textCard(label: string, value: string, onChange: (v: string) => void): HTMLElement {
  const input = el("input", { class: "text-field", type: "text", value }) as HTMLInputElement;
  input.addEventListener("input", () => onChange(input.value.trim()));
  return el("div", { class: "setting-card" }, [
    el("div", { class: "setting-row col" }, [
      el("span", { class: "setting-label" }, [txt(label)]),
      input,
    ]),
  ]);
}

function textareaCard(hint: string, value: string, onChange: (v: string) => void): HTMLElement {
  const ta = el("textarea", { class: "textarea-field", rows: "4", placeholder: hint }) as HTMLTextAreaElement;
  ta.value = value;
  ta.addEventListener("input", () => onChange(ta.value));
  return el("div", { class: "setting-card" }, [ta]);
}

/** Tema uygula: `system` ise tarayıcı tercihine bırak. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const dark = theme === "dark" || (theme === "system" && prefersDark);
  root.classList.toggle("dark", dark);
}
