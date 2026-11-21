// Basit reaktif durum yönetimi. Framework yok; tek bir kaynak (source of truth)
// ve abone listesi. `set` kısmi güncelleme yapar ve aboneleri bilgilendirir.

import type { Account, AppSettings, Msg } from "./types";

export interface AppState {
  accounts: Account[];
  activeAccountId: string | null;
  messages: Msg[];
  settings: AppSettings;
  generating: boolean;
  selectedModel: string;
  supportedModels: string[];
  activeHasApiKey: boolean;
  /** UI yönlendirme: hangi ekran açık. */
  route: Route;
}

export type Route = "chat" | "settings";

type Listener = () => void;

const state: AppState = {
  accounts: [],
  activeAccountId: null,
  messages: [],
  settings: {
    api_url: "https://api.z.ai/api/paas/v4",
    default_model: "glm-5.2",
    temperature: 1.0,
    thinking_enabled: true,
    reasoning_effort: "high",
    system_prompt: "",
    theme: "system",
  },
  generating: false,
  selectedModel: "glm-5.2",
  supportedModels: [],
  activeHasApiKey: false,
  route: "chat",
};

const listeners = new Set<Listener>();

export function getState(): AppState {
  return state;
}

export function setState(patch: Partial<AppState>): void {
  Object.assign(state, patch);
  for (const l of listeners) l();
}

export function subscribe(fn: Listener): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

// ---- Tost (geçici ileti) ----
export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error" | "success";
}
const toasts: Toast[] = [];
const toastListeners = new Set<(t: Toast[]) => void>();
let toastId = 0;

export function getToasts(): Toast[] {
  return toasts;
}
export function subscribeToasts(fn: (t: Toast[]) => void): () => void {
  toastListeners.add(fn);
  return () => toastListeners.delete(fn);
}
export function toast(text: string, kind: Toast["kind"] = "info"): void {
  const t: Toast = { id: ++toastId, text, kind };
  toasts.push(t);
  for (const l of toastListeners) l([...toasts]);
  setTimeout(() => {
    const i = toasts.findIndex((x) => x.id === t.id);
    if (i >= 0) {
      toasts.splice(i, 1);
      for (const l of toastListeners) l([...toasts]);
    }
  }, 3500);
}

// ---- Yardımcılar ----
export function activeAccount(): Account | null {
  return state.accounts.find((a) => a.id === state.activeAccountId) ?? null;
}
