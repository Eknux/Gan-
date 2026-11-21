// Gan v2 frontend girişi. Yükleme, yönlendirme (chat ↔ ayarlar), akış olayı kablolaması.

import {
  activeAccount, getSettings, hasApiKey, listMessages, onStreamEvent,
  supportedModels,
} from "./api";
import { applyTheme, renderSettings } from "./ui/settings";
import { onStreamDelta, onStreamFinalize, onStreamStarted, refreshActiveModel, renderChat } from "./ui/chat";
import { reloadAccounts } from "./ui/accounts";
import { mountToastHost } from "./ui/components";
import { getState, setState, subscribe, toast } from "./state";
import type { StreamEvent } from "./types";
import "./styles.css";

let lastRoute: "chat" | "settings" | null = null;

async function main(): Promise<void> {
  const app = document.getElementById("app")!;
  mountToastHost(app);

  // İlk yükleme: ayarlar, aktif hesap, desteklenen modeller.
  try {
    const settings = await getSettings();
    const acc = await activeAccount();
    const models = await supportedModels().catch(() => ["glm-5.2"]);
    setState({
      settings,
      accounts: [],
      activeAccountId: acc?.id ?? null,
      supportedModels: models,
    });
    applyTheme(settings.theme);
  } catch (e) {
    toast(`Başlangıç hatası: ${e}`, "error");
  }

  // Hesap listesini yükle (aktif hesabı da seçer).
  await reloadAccounts();

  // Aktif hesabın mesajlarını yükle.
  await loadActiveMessages();

  // Akış olaylarını dinle.
  await onStreamEvent(handleStreamEvent);

  // Yönlendirme aboneliği: yalnızca route değişince ekranı değiştir.
  subscribe(renderRoute);
  renderRoute(); // ilk çizim

  // Sohbet ekranı çizildikten sonra aktif modeli yenile.
  if (getState().route === "chat") {
    void refreshActiveModel();
  }
}

/** state değişince çağrılır; yalnızca route değişmişse tüm ekranı yeniden çizer. */
function renderRoute(): void {
  const route = getState().route;
  if (route === lastRoute) return;
  lastRoute = route;
  const app = document.getElementById("app")!;
  if (route === "settings") {
    void renderSettings(app);
  } else {
    renderChat(app);
    void loadActiveMessages().then(() => void refreshActiveModel());
  }
}

/** Aktif hesabın mesajlarını yükle, state'e koy. */
async function loadActiveMessages(): Promise<void> {
  const acc = getState().accounts.find((a) => a.id === getState().activeAccountId);
  if (!acc) {
    setState({ messages: [], generating: false, selectedModel: getState().settings.default_model });
    return;
  }
  try {
    const msgs = await listMessages(acc.id);
    const key = await hasApiKey(acc.id);
    setState({ messages: msgs, activeHasApiKey: key });
  } catch (e) {
    toast(`Mesajlar yüklenemedi: ${e}`, "error");
  }
}

/** Akış olayını işle: state.messages'ı güncelle, chat.ts'in bubble güncelleyicisini çağır. */
function handleStreamEvent(e: StreamEvent): void {
  const state = getState();
  const acc = state.accounts.find((a) => a.id === state.activeAccountId);
  if (!acc || e.account_id !== acc.id) return; // başka hesaba ait olayı yoksay

  switch (e.kind) {
    case "started": {
      const pending = {
        id: e.msg_id,
        role: "assistant" as const,
        text: "",
        thinking: "",
        files: [],
        done: false,
        created_at: Date.now(),
        model: e.model,
      };
      setState({ messages: [...state.messages, pending], generating: true });
      onStreamStarted();
      break;
    }
    case "delta": {
      const updated = state.messages.map((m) =>
        m.id === e.msg_id
          ? { ...m, text: m.text + (e.content ?? ""), thinking: m.thinking + (e.reasoning ?? "") }
          : m,
      );
      setState({ messages: updated });
      onStreamDelta(e.msg_id);
      break;
    }
    case "done": {
      const updated = state.messages.map((m) =>
        m.id === e.msg_id
          ? { ...m, text: e.final_text, thinking: e.final_reasoning, done: true }
          : m,
      );
      setState({ messages: updated, generating: false });
      onStreamFinalize(e.msg_id);
      break;
    }
    case "cancelled": {
      const updated = state.messages.map((m) =>
        m.id === e.msg_id ? { ...m, text: e.partial, done: true } : m,
      );
      setState({ messages: updated, generating: false });
      onStreamFinalize(e.msg_id);
      toast("Yanıt durduruldu");
      break;
    }
    case "error": {
      const updated = state.messages.map((m) =>
        m.id === e.msg_id ? { ...m, done: true, error: e.error } : m,
      );
      setState({ messages: updated, generating: false });
      onStreamFinalize(e.msg_id);
      toast(e.error, "error");
      break;
    }
  }
}

main().catch((e) => {
  console.error("Gan başlatılamadı:", e);
  toast(`Başlatma hatası: ${e}`, "error");
});
