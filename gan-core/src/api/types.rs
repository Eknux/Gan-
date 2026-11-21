//! z.ai chat completions API'nin istek/yanıt tipleri.
//!
//! Tüm tipler `serde` ile serileştirilebilir. Akış parçaları (chunk) için
//! yalnızca kullandığımız alanları çözüyoruz; geri kalanı `serde_json::Value`
//! olarak yok sayılır, böylece z.ai yeni alan eklediğinde kod kırılmaz.

use serde::{Deserialize, Serialize};

use crate::models::ApiConfig;

/// API'ye gönderilen tek mesaj. `reasoning_content` (düşünce) geri gönderilmez;
/// yalnızca görüntüleme amaçlı saklanır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

impl ApiMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// `thinking` parametresi: `{"type": "enabled" | "disabled"}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thinking {
    #[serde(rename = "type")]
    pub kind: String,
}

/// Chat completions istek gövdesi.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    pub stream: bool,
    pub temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl ChatRequest {
    /// Verilen yapılandırmadan ve mesaj listesinden istek gövdesi kur.
    pub fn new(cfg: &ApiConfig, messages: &[ApiMessage], stream: bool) -> Self {
        Self {
            model: cfg.model.clone(),
            messages: messages.to_vec(),
            stream,
            temperature: cfg.temperature,
            max_tokens: cfg.max_tokens,
            thinking: Some(Thinking {
                kind: if cfg.thinking_enabled { "enabled".into() } else { "disabled".into() },
            }),
            reasoning_effort: Some(cfg.reasoning_effort.clone()),
        }
    }
}

/// Akıştan çıkarılan artımlı olay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// Yanıt gövdesine eklenecek metin parçası.
    Content(String),
    /// Düşünce (reasoning) alanına eklenecek metin parçası.
    Reasoning(String),
    /// Akış tamamlandı (`[DONE]` veya `finish_reason` geldi).
    Done,
    /// Kullanıcı tarafından iptal edildi.
    Cancelled,
}

/// z.ai akış chunk'u (OpenAI uyumlu). Yalnızca `choices[].delta` ve
/// `finish_reason` çözülür; geri kalan alan yok sayılır.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    #[serde(default)]
    pub delta: DeltaField,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeltaField {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    /// İlk chunk'ta `role: "assistant"` gelir; yok sayıyoruz.
    #[serde(default)]
    pub role: Option<String>,
}

impl StreamChunk {
    /// Bu chunk'tan çıkarılacak [`Delta`] (varsa).
    pub fn delta(&self) -> Option<Delta> {
        let c = self.choices.first()?;
        if let Some(r) = c.delta.reasoning_content.as_ref().filter(|s| !s.is_empty()) {
            return Some(Delta::Reasoning(r.clone()));
        }
        if let Some(t) = c.delta.content.as_ref().filter(|s| !s.is_empty()) {
            return Some(Delta::Content(t.clone()));
        }
        None
    }

    /// `finish_reason` dolu mu?
    pub fn is_finished(&self) -> bool {
        self.choices
            .first()
            .and_then(|c| c.finish_reason.as_deref())
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_delta() {
        let raw = r#"{"choices":[{"index":0,"delta":{"content":"merhaba"},"finish_reason":null}]}"#;
        let c: StreamChunk = serde_json::from_str(raw).unwrap();
        assert_eq!(c.delta(), Some(Delta::Content("merhaba".into())));
        assert!(!c.is_finished());
    }

    #[test]
    fn parse_reasoning_delta() {
        let raw = r#"{"choices":[{"index":0,"delta":{"reasoning_content":"düşünüyorum"},"finish_reason":null}]}"#;
        let c: StreamChunk = serde_json::from_str(raw).unwrap();
        assert_eq!(c.delta(), Some(Delta::Reasoning("düşünüyorum".into())));
    }

    #[test]
    fn parse_finish() {
        let raw = r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let c: StreamChunk = serde_json::from_str(raw).unwrap();
        assert_eq!(c.delta(), None);
        assert!(c.is_finished());
    }

    #[test]
    fn request_body_serializes_stream_and_thinking() {
        let cfg = ApiConfig {
            base_url: "https://api.z.ai/api/paas/v4".into(),
            api_key: "k".into(),
            model: "glm-5.2".into(),
            temperature: 0.7,
            max_tokens: Some(2048),
            thinking_enabled: true,
            reasoning_effort: "high".into(),
        };
        let req = ChatRequest::new(
            &cfg,
            &[ApiMessage::user("selam")],
            true,
        );
        let j = serde_json::to_value(&req).unwrap();
        assert_eq!(j["stream"], true);
        assert_eq!(j["model"], "glm-5.2");
        assert_eq!(j["temperature"], 0.7);
        assert_eq!(j["max_tokens"], 2048);
        assert_eq!(j["thinking"]["type"], "enabled");
        assert_eq!(j["reasoning_effort"], "high");
    }
}
