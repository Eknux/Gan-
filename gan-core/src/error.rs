//! Hata tipleri.
//!
//! Tüm hatalar tek bir [`Error`] enumunda toplanır; her varyant kullanıcıya
//! anlamlı bir Türkçe ileti eşler ([`Error::user_message`]).

/// Çekirdek hata. UI katmanı `user_message()` ile insancıl bir ileti alır,
/// teknik detayı `Display` üzerinden tutar.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// z.ai API'sinden HTTP hatası (4xx/5xx).
    #[error("z.ai API HTTP hatası: {status} — {body}")]
    ApiHttp { status: u16, body: String },

    /// Ağ hatası (bağlantı reddedildi, DNS, timeout).
    #[error("ağ hatası: {0}")]
    Network(#[from] reqwest::Error),

    /// Akış sırasında JSON parse hatası.
    #[error("akış JSON hatası: {0}")]
    StreamJson(#[from] serde_json::Error),

    /// İstek gövdesi çok büyük / bozuk.
    #[error("istek gövdesi hatası: {0}")]
    Body(String),

    /// API anahtarı eksik.
    #[error("API anahtarı tanımlı değil")]
    MissingApiKey,

    /// API anahtarı şifre çözme hatası.
    #[error("API anahtarı çözülemedi (anahtar/deposit bozuk mu?)")]
    Crypto(String),

    /// Hesap bulunamadı.
    #[error("hesap bulunamadı: {0}")]
    AccountNotFound(String),

    /// Dosya G/Ç hatası (depo oku/yaz).
    #[error("depo G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),

    /// Zaman aşımı.
    #[error("zaman aşımı: {0}")]
    Timeout(String),

    /// İstek iptal edildi (kullanıcı "Durdur" bastı).
    #[error("istek iptal edildi")]
    Cancelled,

    /// Diğer.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Kullanıcıya gösterilecek kısa Türkçe ileti.
    pub fn user_message(&self) -> String {
        match self {
            Error::ApiHttp { status, .. } => match *status {
                401 | 403 => "Kimlik doğrulama başarısız: API anahtarını kontrol et.".into(),
                429 => "Hız sınırına takıldı, biraz bekleyip tekrar dene.".into(),
                500..=599 => "z.ai sunucusu geçici olarak yanıt veremiyor.".into(),
                _ => format!("z.ai API hatası (HTTP {status})."),
            },
            Error::Network(e) => {
                if e.is_timeout() {
                    "İstek zaman aşımına uğradı, bağlantını kontrol et.".into()
                } else if e.is_connect() {
                    "z.ai sunucusuna bağlanılamadı, internet bağlantını kontrol et.".into()
                } else {
                    "Bağlantı hatası, internet bağlantını kontrol et.".into()
                }
            }
            Error::MissingApiKey => "Önce Ayarlar'dan bir API anahtarı gir.".into(),
            Error::Cancelled => "Yanıt durduruldu.".into(),
            Error::AccountNotFound(id) => format!("Hesap bulunamadı: {id}"),
            Error::Timeout(_) => "Yanıt başlamadı (zaman aşımı).".into(),
            Error::Crypto(_) => "API anahtarı çözülemedi, yeniden gir.".into(),
            Error::StreamJson(_) => "Akış biçimi bozuk, model değiştirmeyi dene.".into(),
            other => format!("{other}"),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // thiserror `#[error(...)]` atribütlerinden `Display` üretir; biz de onu kullanırız.
        s.serialize_str(&self.to_string())
    }
}
