//! Veri modelleri.
//!
//! Eski Kotlin Gan'deki `Account`, `Msg`, `AppSettings` tiplerinin Rust karşılığıdır;
//! ancak web kazıma (DOM scraping) yerine resmi z.ai API'sini kullandığımız için
//! `site` / `sel` (seçici) alanları kaldırılmış, yerine `api_url`, `default_model`,
//! `temperature`, `thinking` gibi API seçenekleri eklenmiştir.

use serde::{Deserialize, Serialize};

/// Hesap kimliği (UUID v4, tireler olmadan).
pub type AccountId = String;
/// Mesaj kimliği (monoton artan,Epoch millis tabanlı).
pub type MsgId = u64;

/// Z.ai tarafından desteklenen metin modelleri.
///
/// Liste `https://docs.z.ai/api-reference/llm/chat-completion` üzerinden doğrulanmıştır.
/// Yeni modeller çıktığında bu listeye ekleyin; frontend seçici liste için
/// [`SUPPORTED_MODELS`] sabitini kullanır.
pub const SUPPORTED_MODELS: &[&str] = &[
    "glm-5.3",
    "glm-5.2",
    "glm-5.1",
    "glm-5",
    "glm-4.7",
    "glm-4.6",
];

/// Varsayılan API kök adresi.
pub const DEFAULT_API_URL: &str = "https://api.z.ai/api/paas/v4";
/// Varsayılan model (dengeli hız/kalite).
pub const DEFAULT_MODEL: &str = "glm-5.2";

/// Bir Z.ai hesabı. Eski `chatUrl` alanı yok; onun yerine her hesabın kendi
/// API anahtarı (şifreli, [`crate::crypto`] üzerinden) ve tercih ettiği modeli vardır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    /// ARGB renk (avatar arka planı). Eski palet değerlerinden biri.
    pub color: u32,
    /// Oluşturulma zamanı, Unix epoch ms.
    pub created_at: i64,
    /// Hesaba özel tercih edilen model; `None` ise genel ayar kullanılır.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Hesaba özel sistem promptu; `None` ise genel ayar kullanılır.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

impl Account {
    /// Yeni hesap oluştur. Kimlik `uuid::Uuid::new_v4` ile üretilir.
    pub fn new(name: String, color: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name: name.trim().to_string(),
            color,
            created_at: now_millis(),
            model: None,
            system_prompt: None,
        }
    }

    /// Avatar için kullanılacak baş harf.
    pub fn initial(&self) -> String {
        self.name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into())
    }
}

/// Bir mesaj içindeki dosya referansı (z.ai tarafından üretilen dosyalar için).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FileRef {
    pub name: String,
    pub url: String,
}

/// Mesaj rolü.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MsgRole {
    User,
    Assistant,
    System,
}

impl MsgRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MsgRole::User => "user",
            MsgRole::Assistant => "assistant",
            MsgRole::System => "system",
        }
    }
}

/// Sohbet mesajı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Msg {
    pub id: MsgId,
    pub role: MsgRole,
    /// Markdown yanıt gövdesi.
    pub text: String,
    /// Modelin "düşünce" (reasoning) çıktısı.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thinking: String,
    /// Cevaba ekli dosyalar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileRef>,
    /// Yanıt tamamlandı mı? (akış devam ediyorsa `false`).
    #[serde(default = "default_true")]
    pub done: bool,
    /// Oluşturulma zamanı, Unix epoch ms.
    pub created_at: i64,
    /// Asistan mesajları için: hangi model üretti.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Varsa hata iletisi (akış kırılırsa doldurulur).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Msg {
    /// Yeni kullanıcı mesajı.
    pub fn user(id: MsgId, text: impl Into<String>) -> Self {
        Self {
            id,
            role: MsgRole::User,
            text: text.into(),
            thinking: String::new(),
            files: Vec::new(),
            done: true,
            created_at: now_millis(),
            model: None,
            error: None,
        }
    }

    /// Boş, devam eden asistan mesajı (akış doldurur).
    pub fn assistant_pending(id: MsgId, model: Option<String>) -> Self {
        Self {
            id,
            role: MsgRole::Assistant,
            text: String::new(),
            thinking: String::new(),
            files: Vec::new(),
            done: false,
            created_at: now_millis(),
            model,
            error: None,
        }
    }
}

/// Tema tercihi.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Sistem temasını izle.
    #[default]
    System,
    Light,
    Dark,
}

/// Genel uygulama ayarları. API anahtarı hesap başına saklandığı için burada değil;
/// bu yapı yalnızca hesaptan bağımsız tercihleri tutar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// API kök adresi. Varsayılan [`DEFAULT_API_URL`].
    pub api_url: String,
    /// Yeni hesaplarda/seçimlerde kullanılacak varsayılan model.
    pub default_model: String,
    /// Örnekleme sıcaklığı (0.0–1.0).
    pub temperature: f64,
    /// Maksimum üretim token sayısı; `None` = model varsayılanı.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Düşünme (chain-of-thought) etkin mi?
    #[serde(default = "default_true")]
    pub thinking_enabled: bool,
    /// Düşünme derinliği: `max|xhigh|high|medium|low|minimal|none`.
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,
    /// Genel sistem promptu (hesabın kendi yoksa kullanılır).
    #[serde(default)]
    pub system_prompt: String,
    /// Genel tema.
    #[serde(default)]
    pub theme: Theme,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_url: DEFAULT_API_URL.into(),
            default_model: DEFAULT_MODEL.into(),
            temperature: 1.0,
            max_tokens: None,
            thinking_enabled: true,
            reasoning_effort: "high".into(),
            system_prompt: String::new(),
            theme: Theme::System,
        }
    }
}

/// API bağlantı yapılandırması (bir istek anında derlenir).
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: Option<u32>,
    pub thinking_enabled: bool,
    pub reasoning_effort: String,
}

impl ApiConfig {
    /// Tam chat completions uç adresi.
    pub fn chat_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{base}/chat/completions")
    }
}

// ---- yardımcılar ----

fn default_true() -> bool {
    true
}
fn default_reasoning_effort() -> String {
    "high".into()
}

/// Şu anki Unix epoch milisaniyesi.
pub fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Avatar paleti (eski Kotlin Gan ile aynı değerler).
pub const AVATAR_PALETTE: &[u32] = &[
    0xFF6750A4, 0xFF006A6A, 0xFFB3261E, 0xFF386A20, 0xFF9C4146, 0xFF005AC1,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_has_uuid_id() {
        let a = Account::new("test".into(), AVATAR_PALETTE[0]);
        assert_eq!(a.id.len(), 32); // uuid v4 simple, tire yok
        assert_eq!(a.initial(), "T");
    }

    #[test]
    fn settings_default_matches_docs() {
        let s = AppSettings::default();
        assert_eq!(s.api_url, DEFAULT_API_URL);
        assert_eq!(s.default_model, DEFAULT_MODEL);
        assert!((s.temperature - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn msg_serde_roundtrip() {
        let m = Msg::user(1, "merhaba");
        let j = serde_json::to_string(&m).unwrap();
        let m2: Msg = serde_json::from_str(&j).unwrap();
        assert_eq!(m2.text, "merhaba");
        assert_eq!(m2.role, MsgRole::User);
    }

    #[test]
    fn api_url_no_trailing_slash() {
        let cfg = ApiConfig {
            base_url: "https://api.z.ai/api/paas/v4/".into(),
            api_key: "k".into(),
            model: "glm-5.2".into(),
            temperature: 1.0,
            max_tokens: None,
            thinking_enabled: true,
            reasoning_effort: "high".into(),
        };
        assert_eq!(
            cfg.chat_url(),
            "https://api.z.ai/api/paas/v4/chat/completions"
        );
    }
}
