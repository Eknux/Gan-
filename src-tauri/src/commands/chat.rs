//! Sohbet komutları: gönder, durdur, yeni sohbet, mesajları listele.

use tauri::State;

use gan_core::{error::Error, Msg, MsgId};

use crate::state::AppState;

fn emsg(e: Error) -> String {
    e.user_message()
}

/// Mesaj gönder. Hemen döner; asistan yanıtı `gan-stream` olaylarıyla gelir.
/// Dönüş değeri, akışın dolduracağı asistan mesajının kimliğidir (UI bunu
/// takip edip ilgili mesajı günceller).
#[tauri::command]
pub fn send_message(
    account_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<MsgId, String> {
    state.sessions.send(&account_id, &text).map_err(emsg)
}

/// Üretimi durdur.
#[tauri::command]
pub fn stop(account_id: String, state: State<'_, AppState>) {
    state.sessions.stop(&account_id);
}

/// Yeni sohbet: mesaj geçmişini temizle, devam eden isteği iptal et.
#[tauri::command]
pub fn new_chat(account_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.sessions.new_chat(&account_id).map_err(emsg)
}

/// Bir hesabın mesaj geçmişini listele (bellek aynası).
#[tauri::command]
pub fn list_messages(account_id: String, state: State<'_, AppState>) -> Vec<Msg> {
    state.sessions.messages(&account_id)
}

/// Hesap şu an yanıt üretiyor mu?
#[tauri::command]
pub fn is_generating(account_id: String, state: State<'_, AppState>) -> bool {
    state.sessions.is_generating(&account_id)
}
