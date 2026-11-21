//! Gan v2 çekirdeği.
//!
//! Bu crate Tauri'ye (veya herhangi bir UI çerçevesine) bağımlı değildir;
//! z.ai chat completions API'si ile konuşan saf Rust kütüphanesidir.
//! Üç katmandan oluşur:
//!
//! * [`api`] — HTTP istemcisi + SSE akış çözümleyicisi (z.ai GLM uç noktası).
//! * [`store`] — Hesapları, mesajları ve ayarları yerel JSON dosyasına atomik yazan kalıcı depo.
//! * [`session`] — Hesap başına oturum durumu, devam eden istekleri iptal edebilme,
//!   akış olaylarını [`session::StreamEvent`] olarak yayınlama.
//!
//! API anahtarları rest'te AES-256-GCM ile şifrelenir ([`crypto`]).
//!
//! Mimarinin tamamı için projenin kök dizinindeki `ARCHITECTURE.md` dosyasına bakın.

pub mod api;
pub mod crypto;
pub mod error;
pub mod models;
pub mod session;
pub mod store;

pub use error::{Error, Result};
pub use models::{
    Account, AccountId, ApiConfig, AppSettings, FileRef, Msg, MsgId, MsgRole, Theme,
};
pub use session::{SessionManager, StreamEvent};
pub use store::Store;
pub use api::Client;
