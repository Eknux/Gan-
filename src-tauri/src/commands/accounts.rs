//! Hesap yönetimi komutları.

use tauri::State;

use gan_core::{error::Error, Account, AccountId};

use crate::state::AppState;

/// Hesabın kullanıcıya gösterilecek iletiselle hata dizesi.
fn emsg(e: Error) -> String {
    e.user_message()
}

/// Tüm hesapları listele.
#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> Vec<Account> {
    state.store.accounts()
}

/// Yeni hesap ekle ve aktif yap. Eklenen hesabı döndür.
#[tauri::command]
pub fn add_account(name: String, state: State<'_, AppState>) -> Result<Account, String> {
    state.store.add_account(name).map_err(emsg)
}

/// Hesabı (ve mesaj/anahtar geçmişini) sil.
#[tauri::command]
pub fn remove_account(id: AccountId, state: State<'_, AppState>) -> Result<(), String> {
    state.sessions.drop_session(&id);
    state.store.remove_account(&id).map_err(emsg)
}

/// Hesabı aktif yap; oturumunu belleğe yükle.
#[tauri::command]
pub fn select_account(id: AccountId, state: State<'_, AppState>) -> Result<(), String> {
    state.store.set_active(Some(id.clone())).map_err(emsg)?;
    state.sessions.ensure(&id).map_err(emsg)
}

/// Aktif hesabı döndür.
#[tauri::command]
pub fn active_account(state: State<'_, AppState>) -> Option<Account> {
    state
        .store
        .active_id()
        .and_then(|id| state.store.accounts().into_iter().find(|a| a.id == id))
}

/// Hesabın tercih ettiği modeli ayarla.
#[tauri::command]
pub fn set_account_model(
    id: AccountId,
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .sessions
        .set_model(&id, model)
        .map_err(emsg)
}

/// Hesaba özel sistem promptunu ayarla.
#[tauri::command]
pub fn set_account_system_prompt(
    id: AccountId,
    system_prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Doğrudan depo üzerinden yaz; çekirdek henüz ayrı bir API sunmuyor.
    state
        .store
        .set_account_system_prompt(&id, system_prompt.filter(|s| !s.is_empty()))
        .map_err(emsg)
}

/// Bir hesabın API anahtarını kaydet (şifrelenmiş olarak).
#[tauri::command]
pub fn set_api_key(id: AccountId, api_key: String, state: State<'_, AppState>) -> Result<(), String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("API anahtarı boş olamaz".into());
    }
    state.store.set_api_key(&id, key).map_err(emsg)
}

/// Hesabın API anahtarı tanımlı mı?
#[tauri::command]
pub fn has_api_key(id: AccountId, state: State<'_, AppState>) -> bool {
    state.store.has_api_key(&id)
}
