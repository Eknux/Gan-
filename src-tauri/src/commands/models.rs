//! Model komutları.

use tauri::State;

use gan_core::{error::Error, models::SUPPORTED_MODELS};

use crate::state::AppState;

fn emsg(e: Error) -> String {
    e.user_message()
}

/// z.ai tarafından desteklenen statik model listesi.
/// (Canlı liste API'den de alınabilir, ancak uç nokta kararlı olduğu için
/// statik tutmak daha hızlı ve çevrimdışı çalışır.)
#[tauri::command]
pub fn supported_models() -> Vec<String> {
    SUPPORTED_MODELS.iter().map(|s| s.to_string()).collect()
}

/// Bir hesabın seçili modelini döndür (hesap tercihi > genel ayar).
#[tauri::command]
pub fn account_model(account_id: String, state: State<'_, AppState>) -> Result<String, String> {
    state.sessions.model(&account_id).map_err(emsg)
}
