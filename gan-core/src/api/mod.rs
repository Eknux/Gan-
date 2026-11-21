//! z.ai (GLM) chat completions API katmanı.
//!
//! Üç parçadan oluşur:
//! * [`types`] — İstek gövdesi ve akış parçası (chunk) tipleri.
//! * [`client`] — `reqwest` tabanlı HTTP istemcisi; bağlantı havuzlu, iptal edilebilir.
//! * [`stream`] — SSE akış çözümleyicisi; `data:` satırlarını [`types::Delta`]'ya ayrıştırır.
//!
//! Resmi belge: <https://docs.z.ai/api-reference/llm/chat-completion>
//! Kök adres: `https://api.z.ai/api/paas/v4` · Uç: `POST /chat/completions`
//! Kimlik doğrulama: `Authorization: Bearer <API_KEY>`

pub mod client;
pub mod stream;
pub mod types;

pub use client::Client;
pub use types::{ApiMessage, ChatRequest, Delta, StreamChunk};
