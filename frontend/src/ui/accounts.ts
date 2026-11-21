// Hesap yönetimi: alt sayfa (liste/seç/sil) ve "hesap ekle" penceresi.

import { addAccount, listAccounts, removeAccount, selectAccount, setApiKey } from "../api";
import { getState, setState, toast } from "../state";
import { avatar, bottomSheet, confirmDialog, el, ganAvatar, icon, txt } from "./components";

/** Hesap listesini ve aktif hesabı yeniden yükle, state'i güncelle. */
export async function reloadAccounts(): Promise<void> {
  const accounts = await listAccounts().catch(() => []);
  const activeId = getState().activeAccountId;
  // Aktif hesap artık listede değilse ilk hesabı seç.
  if (activeId && !accounts.some((a) => a.id === activeId)) {
    const first = accounts[0];
    if (first) await selectAccount(first.id).catch(() => {});
    else setState({ activeAccountId: null });
  } else if (!activeId && accounts.length > 0) {
    await selectAccount(accounts[0].id).catch(() => {});
  }
  setState({ accounts });
}

/** Hesap alt sayfası: liste + seç + sil + ekle. */
export async function openAccountSheet(): Promise<void> {
  const accounts = await listAccounts().catch(() => []);
  const activeId = getState().activeAccountId;
  const content = el("div", { class: "sheet-list" });

  if (accounts.length === 0) {
    content.append(el("p", { class: "sheet-empty" }, [txt("Henüz hesap yok. Aşağıdan ekle.")]));
  }
  for (const a of accounts) {
    const row = el("div", { class: `account-row ${a.id === activeId ? "active" : ""}` }, [
      avatar(a.name, a.color, 40),
      el("div", { class: "account-info" }, [
        el("div", { class: "account-row-name" }, [txt(a.name)]),
        el("div", { class: "account-row-sub" }, [txt(a.id === activeId ? "Aktif" : "Seçmek için dokun")]),
      ]),
      el("button", {
        class: "icon-btn danger",
        title: "Sil",
        onclick: () =>
          confirmDialog(
            `${a.name} silinsin mi?`,
            "Hesabın API anahtarı ve yerel sohbet geçmişi bu cihazdan silinir.",
            async () => {
              try {
                await removeAccount(a.id);
                await reloadAccounts();
                close();
                if (getState().accounts.length > 0) void openAccountSheet();
              } catch (e) {
                toast(`Silinemedi: ${e}`, "error");
              }
            },
          ),
      }, [icon("trash", 18)]),
    ]);
    row.addEventListener("click", async (e) => {
      if ((e.target as HTMLElement).closest("button")) return;
      try {
        await selectAccount(a.id);
        await reloadAccounts();
        close();
      } catch (err) {
        toast(`Seçilemedi: ${err}`, "error");
      }
    });
    content.append(row);
  }

  content.append(
    el("button", { class: "btn btn-primary block", onclick: () => { close(); openAddAccountDialog(); } }, [
      icon("plus", 18), txt("Hesap ekle"),
    ]),
  );

  // `bottomSheet` bir kapatma fonksiyonu döndürür; tüm handler'lar bunu kullanır.
  const close = bottomSheet("Hesaplar", content);
}

/** "Hesap ekle" penceresi: ad + API anahtarı birlikte girilir. */
export function openAddAccountDialog(): void {
  const overlay = el("div", { class: "dialog-overlay" });
  const card = el("div", { class: "dialog wide" });
  const nameInput = el("input", { class: "text-field", type: "text", placeholder: "Hesap adı (ör. İş)" });
  const keyInput = el("input", {
    class: "text-field",
    type: "password",
    placeholder: "z.ai API anahtarı (sk-...)",
    autocomplete: "off",
    spellcheck: "false",
  });
  const helpLink = el("a", {
    class: "help-link",
    href: "https://z.ai/model-api",
    target: "_blank",
    rel: "noopener noreferrer",
  }, [txt("z.ai'den API anahtarı al →")]);
  card.append(
    el("h3", {}, [txt("Yeni hesap")]),
    el("p", { class: "dialog-sub" }, [txt("Ad ve API anahtarı birlikte girilir; anahtar cihazda şifrelenir.")]),
    el("label", { class: "field-label" }, [txt("Hesap adı")]),
    nameInput,
    el("label", { class: "field-label" }, [txt("API anahtarı")]),
    keyInput,
    helpLink,
    el("div", { class: "dialog-actions" }, [
      el("button", { class: "btn btn-text", onclick: close }, [txt("Vazgeç")]),
      el("button", { class: "btn btn-primary", onclick: () => void onCreate() }, [txt("Oluştur")]),
    ]),
  );
  overlay.append(card);
  overlay.addEventListener("click", (e) => { if (e.target === overlay) close(); });
  document.body.append(overlay);
  requestAnimationFrame(() => overlay.classList.add("open"));
  nameInput.focus();

  function close(): void {
    overlay.classList.remove("open");
    setTimeout(() => overlay.remove(), 180);
  }
  async function onCreate(): Promise<void> {
    const name = nameInput.value.trim();
    const key = keyInput.value.trim();
    if (!name) { toast("Hesap adı gerekli", "error"); return; }
    if (!key) { toast("API anahtarı gerekli", "error"); return; }
    try {
      const acc = await addAccount(name);
      await setApiKey(acc.id, key);
      await reloadAccounts();
      close();
      toast("Hesap eklendi", "success");
    } catch (e) {
      toast(`Eklenemedi: ${e}`, "error");
    }
  }
}

export { ganAvatar };
