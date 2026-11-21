//! Uygulama durumu. Tauri'nin yönetimine (`app.manage`) verilir; komutlar
//! [`tauri::State`] ile erişir.

use std::sync::Arc;

use gan_core::{session::SessionManager, Store};

pub struct AppState {
    pub store: Arc<Store>,
    pub sessions: SessionManager,
}
