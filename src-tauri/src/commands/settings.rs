//! Ayar komutları.

use tauri::State;

use gan_core::{error::Error, AppSettings};

use crate::state::AppState;

fn emsg(e: Error) -> String {
    e.user_message()
}

/// Genel ayarları oku.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.store.settings()
}

/// Genel ayarları kaydet.
#[tauri::command]
pub fn save_settings(settings: AppSettings, state: State<'_, AppState>) -> Result<(), String> {
    // Model geçerliliğini basitçe kontrol et.
    if settings.default_model.is_empty() {
        return Err("Varsayılan model boş olamaz".into());
    }
    if !(0.0..=1.0).contains(&settings.temperature) {
        return Err("Sıcaklık 0.0–1.0 aralığında olmalı".into());
    }
    state.store.save_settings(settings).map_err(emsg)
}

/// Bir hesabın API anahtarını doğrula (küçük bir istek atar).
/// Asenkron; frontend `await invoke('test_api_key', {...})` ile bekler.
#[tauri::command]
pub async fn test_api_key(account_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .sessions
        .test_api_key(&account_id)
        .await
        .map_err(emsg)
}
