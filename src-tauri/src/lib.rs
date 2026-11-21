//! Gan v2 — Tauri 2 uygulama girişi.
//!
//! [`run`] hem masaüstü hem mobil (Android/iOS) için tek giriş noktasıdır.
//! `mobile_entry_point` attribute'u yalnızca mobil hedeflerde etkindir ve
//! fonksiyonu JNI giriş noktasına dönüştürür; masaüstünde no-op'tur.
//!
//! Kurulum adımları:
//! 1. Uygulama veri dizinini çöz (`app_data_dir`).
//! 2. [`Store`] ve [`Client`] oluştur.
//! 3. [`TauriSink`] (akış olaylarını frontend'e iletir) ile [`SessionManager`] kur.
//! 4. [`AppState`] yönetimine al.
//! 5. Tüm komutları kaydet.

use std::sync::Arc;

use tauri::Manager;

use gan_core::{session::SessionManager, Client, Store};

mod commands;
mod sink;
mod state;

use sink::TauriSink;
use state::AppState;

/// `log` crate'i için ilkel bir köprü: Tauri'nin log hedefine yazmak yerine
/// stderr'a döker. Üretimde `tauri-plugin-log` eklenebilir; şimdilik bu yeter.
struct StderrLog;
impl log::Log for StderrLog {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        eprintln!("[{}] {}", r.level(), r.args());
    }
    fn flush(&self) {}
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = log::set_logger(&StderrLog);
    log::set_max_level(log::LevelFilter::Info);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Uygulama veri dizini: Android'de uygulamanın iç dosya dizini,
            // masaüstünde platforma özgü uygulama veri dizini.
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| {
                    log::error!("app_data_dir çözülemedi: {e}");
                    Box::<dyn std::error::Error>::from(format!("app_data_dir: {e}"))
                })?;
            log::info!("Gan veri dizini: {}", data_dir.display());

            let store = Arc::new(Store::open(&data_dir).map_err(|e| {
                log::error!("depoyu açma başarısız: {e}");
                Box::<dyn std::error::Error>::from(format!("store open: {e}"))
            })?);
            let client = Client::new();
            let sink = Arc::new(TauriSink {
                handle: app.handle().clone(),
            }) as Arc<dyn gan_core::session::EventSink>;
            let sessions = SessionManager::new(store.clone(), client, sink);

            app.manage(AppState { store, sessions });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Hesaplar
            commands::accounts::list_accounts,
            commands::accounts::add_account,
            commands::accounts::remove_account,
            commands::accounts::select_account,
            commands::accounts::active_account,
            commands::accounts::set_account_model,
            commands::accounts::set_account_system_prompt,
            commands::accounts::set_api_key,
            commands::accounts::has_api_key,
            // Sohbet
            commands::chat::send_message,
            commands::chat::stop,
            commands::chat::new_chat,
            commands::chat::list_messages,
            commands::chat::is_generating,
            // Modeller
            commands::models::supported_models,
            commands::models::account_model,
            // Ayarlar
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::test_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("Gan çalıştırılırken hata");
}
