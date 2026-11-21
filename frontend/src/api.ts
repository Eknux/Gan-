// Tauri komut sarmalayıcıları + akış olayı dinleyicisi.
// Tüm invoke çağrıları buradan geçer; hatalar统一的 olarak yakalanır.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Account, AppSettings, Msg, StreamEvent } from "./types";

// ---- Hesaplar ----
export const listAccounts = (): Promise<Account[]> => invoke("list_accounts");
export const addAccount = (name: string): Promise<Account> => invoke("add_account", { name });
export const removeAccount = (id: string): Promise<void> => invoke("remove_account", { id });
export const selectAccount = (id: string): Promise<void> => invoke("select_account", { id });
export const activeAccount = (): Promise<Account | null> => invoke("active_account");
export const setAccountModel = (id: string, model: string | null): Promise<void> =>
  invoke("set_account_model", { id, model });
export const setAccountSystemPrompt = (id: string, systemPrompt: string | null): Promise<void> =>
  invoke("set_account_system_prompt", { id, systemPrompt });
export const setApiKey = (id: string, apiKey: string): Promise<void> =>
  invoke("set_api_key", { id, apiKey });
export const hasApiKey = (id: string): Promise<boolean> => invoke("has_api_key", { id });

// ---- Sohbet ----
export const sendMessage = (accountId: string, text: string): Promise<number> =>
  invoke("send_message", { accountId, text });
export const stopGeneration = (accountId: string): Promise<void> =>
  invoke("stop", { accountId });
export const newChat = (accountId: string): Promise<void> => invoke("new_chat", { accountId });
export const listMessages = (accountId: string): Promise<Msg[]> =>
  invoke("list_messages", { accountId });
export const isGenerating = (accountId: string): Promise<boolean> =>
  invoke("is_generating", { accountId });

// ---- Modeller ----
export const supportedModels = (): Promise<string[]> => invoke("supported_models");
export const accountModel = (accountId: string): Promise<string> =>
  invoke("account_model", { accountId });

// ---- Ayarlar ----
export const getSettings = (): Promise<AppSettings> => invoke("get_settings");
export const saveSettings = (settings: AppSettings): Promise<void> =>
  invoke("save_settings", { settings });
export const testApiKey = (accountId: string): Promise<void> =>
  invoke("test_api_key", { accountId });

// ---- Akış olayları ----
/** `gan-stream` olaylarını dinle. Dönüş: kapatma fonksiyonu. */
export function onStreamEvent(handler: (e: StreamEvent) => void): Promise<UnlistenFn> {
  return listen<StreamEvent>("gan-stream", (ev) => handler(ev.payload));
}
