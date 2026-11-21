//! Tauri komutları. UI katmanı `invoke('komut_adi', {...})` ile çağırır.
//!
//! Hata yönetimi: çekirdek `Error`'u `String`'e eşler (`user_message`),
//! böylece frontend basit bir hata dizesi alır.

pub mod accounts;
pub mod chat;
pub mod models;
pub mod settings;
