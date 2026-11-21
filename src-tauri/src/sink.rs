//! [`gan_core::session::EventSink`] uygulaması: akış olaylarını Tauri
//! olaylarına (`gan://stream`) dönüştürür ve frontend'e yayar.
//!
//! Frontend `@tauri-apps/api/event` üzerinden `listen('gan://stream', ...)`
//! ile bu olayları dinler.

use gan_core::session::{EventSink, StreamEvent};
use tauri::{AppHandle, Emitter};

pub struct TauriSink {
    pub handle: AppHandle,
}

impl EventSink for TauriSink {
    fn emit(&self, event: StreamEvent) {
        // Olay adında `://` Tauri'de sorun çıkarmasın diye tire kullanıyoruz;
        // frontend `listen('gan-stream', ...)` ile dinler.
        if let Err(e) = self.handle.emit("gan-stream", &event) {
            log::warn!("akış olayı yayınamadı: {e}");
        }
    }
}
