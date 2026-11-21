//! Oturum yöneticisi.
//!
//! [`Store`] (kalıcılık) ve [`crate::api::Client`] (HTTP) katmanlarını
//! birleştirir, hesap başına canlı oturum durunu tutar:
//!
//! * Mesaj geçmişinin bellekte aynası (UI için hızlı okuma).
//! * Devam eden akış isteği için [`CancellationToken`] (kullanıcı "Durdur").
//! * Akış olaylarını [`EventSink`] üzerinden UI'ya yayınlama.
//!
//! `send` hemen döner; asıl akış arka plan görevinde yürür ve olayları
//! `EventSink::emit` ile bildirir. Bu sayede UI hiçbir zaman HTTP
//! isteğini beklemez.
//!
//! ## Olay akışı
//!
//! ```text
//! send() → [Görev başlar] → Started → Delta* → (Done | Cancelled | Error)
//! ```
//!
//! Her olay, ilgili asistan mesajının kimliğini (`msg_id`) taşır; UI bu
//! kimlikle mesajı günceller.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::api::{ApiMessage, Client, Delta};
use crate::error::{Error, Result};
use crate::models::{ApiConfig, Account, AccountId, AppSettings, Msg, MsgId, MsgRole, now_millis};
use crate::store::Store;

/// Akış olayı. UI bu olayları dinleyerek mesaj listesini günceller.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Yeni asistan mesajı oluşturuldu, akış başlıyor.
    Started { account_id: AccountId, msg_id: MsgId, model: String },
    /// Artımlı güncelleme: `content` ve/veya `reasoning` parçası geldi.
    /// Her ikisi de `Some` ise sırayla eklenmelidir.
    Delta {
        account_id: AccountId,
        msg_id: MsgId,
        content: Option<String>,
        reasoning: Option<String>,
    },
    /// Akış başarıyla tamamlandı. `final_text`/`final_reasoning` tam metindir.
    Done {
        account_id: AccountId,
        msg_id: MsgId,
        final_text: String,
        final_reasoning: String,
    },
    /// Kullanıcı durdurdu. `partial` o ana kadar gelen metindir.
    Cancelled { account_id: AccountId, msg_id: MsgId, partial: String },
    /// Hata. `error` kullanıcıya gösterilecek ileti.
    Error { account_id: AccountId, msg_id: MsgId, error: String },
}

impl StreamEvent {
    pub fn account_id(&self) -> &str {
        match self {
            StreamEvent::Started { account_id, .. }
            | StreamEvent::Delta { account_id, .. }
            | StreamEvent::Done { account_id, .. }
            | StreamEvent::Cancelled { account_id, .. }
            | StreamEvent::Error { account_id, .. } => account_id,
        }
    }
}

/// Olay hedefi. Tauri katmanı bunu uygular ve olayları `app_handle.emit`
/// ile frontend'e geçirir.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: StreamEvent);
}

/// Bir hesabın canlı oturum durumu.
struct AccountSession {
    /// Bellekte mesaj aynası. Kalıcılık `Store` üzerinden yapılır.
    messages: Vec<Msg>,
    /// Devam eden istek varsa iptal tokeni.
    cancel: Option<CancellationToken>,
    /// Sonraki mesaj kimliği (monoton).
    next_id: AtomicU64,
}

impl AccountSession {
    fn new(initial: Vec<Msg>) -> Self {
        let max_id = initial.iter().map(|m| m.id).max().unwrap_or(0);
        let base = (now_millis().max(1) as u64).max(max_id + 1);
        Self {
            messages: initial,
            cancel: None,
            next_id: AtomicU64::new(base),
        }
    }

    fn alloc_id(&self) -> MsgId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn is_generating(&self) -> bool {
        self.cancel.is_some()
    }
}

/// Oturum yöneticisi. Clone ucuzdur (içerik `Arc`).
#[derive(Clone)]
pub struct SessionManager {
    store: Arc<Store>,
    client: Client,
    sink: Arc<dyn EventSink>,
    sessions: Arc<Mutex<HashMap<AccountId, Arc<Mutex<AccountSession>>>>>,
}

impl SessionManager {
    /// Yeni yönetici. `sink` akış olaylarını alır.
    pub fn new(store: Arc<Store>, client: Client, sink: Arc<dyn EventSink>) -> Self {
        Self {
            store,
            client,
            sink,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Bir hesabın oturumunu yüklenmiş olarak işaretle (mesajları storedan belleğe al).
    /// Hesap seçilince çağrılır.
    pub fn ensure(&self, account_id: &str) -> Result<()> {
        let exists = self.store.accounts().iter().any(|a| a.id == account_id);
        if !exists {
            return Err(Error::AccountNotFound(account_id.into()));
        }
        let mut map = self.sessions.lock().unwrap();
        map.entry(account_id.to_string())
            .or_insert_with(|| {
                let msgs = self.store.messages(account_id);
                Arc::new(Mutex::new(AccountSession::new(msgs)))
            });
        Ok(())
    }

    /// Oturumu bellekten bırak (hesap silinince / çıkışta). Devam eden
    /// istek varsa iptal edilir.
    pub fn drop_session(&self, account_id: &str) {
        let session = {
            let mut map = self.sessions.lock().unwrap();
            map.remove(account_id)
        };
        if let Some(s) = session {
            if let Some(c) = s.lock().unwrap().cancel.take() {
                c.cancel();
            }
        }
    }

    /// Bir hesabın mesajlarını oku (bellek aynası; yoksa storedan yükler).
    pub fn messages(&self, account_id: &str) -> Vec<Msg> {
        // Önce belleğe al (ensure).
        let _ = self.ensure(account_id);
        let map = self.sessions.lock().unwrap();
        map.get(account_id)
            .map(|s| s.lock().unwrap().messages.clone())
            .unwrap_or_default()
    }

    /// Bu hesap şu an yanıt üretiyor mu?
    pub fn is_generating(&self, account_id: &str) -> bool {
        let map = self.sessions.lock().unwrap();
        map.get(account_id)
            .map(|s| s.lock().unwrap().is_generating())
            .unwrap_or(false)
    }

    /// Yeni mesaj gönder. Hemen döner; yanıt olaylarla gelir.
    ///
    /// Hata durumlarında (anahtar eksik, zaten üretiyor, boş metin) `Err` döner
    /// ve hiçbir mesaj eklenmez.
    pub fn send(&self, account_id: &str, text: &str) -> Result<MsgId> {
        let text = text.trim();
        if text.is_empty() {
            return Err(Error::Other("boş mesaj".into()));
        }
        self.ensure(account_id)?;

        // Hesabı ve API anahtarını al.
        let account = self
            .store
            .accounts()
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| Error::AccountNotFound(account_id.into()))?;
        let api_key = self
            .store
            .api_key(account_id)?
            .ok_or(Error::MissingApiKey)?;

        // Oturum kilidini al, kullanıcı + bekleyen asistan mesajlarını ekle.
        let session = {
            let map = self.sessions.lock().unwrap();
            map.get(account_id)
                .cloned()
                .ok_or_else(|| Error::AccountNotFound(account_id.into()))?
        };

        let mut guard = session.lock().unwrap();
        if guard.is_generating() {
            return Err(Error::Other("zaten bir yanıt üretiliyor".into()));
        }

        let user_id = guard.alloc_id();
        let asst_id = guard.alloc_id();
        let user_msg = Msg::user(user_id, text);
        // API'ye gönderilecek mesaj dizisini burada, kilidi tutarken kur.
        let settings = self.store.settings();
        let model = resolve_model(&account, &settings);
        let history = build_api_messages(&guard.messages, &user_msg, &account, &settings);
        guard.messages.push(user_msg);
        let pending = Msg::assistant_pending(asst_id, Some(model.clone()));
        guard.messages.push(pending.clone());

        // CancellationToken oluştur ve oturuma kaydet.
        let cancel = CancellationToken::new();
        guard.cancel = Some(cancel.clone());
        drop(guard);

        // Kalıcı depoya kullanıcı mesajını yaz (asistan mesajı akış bitince yazılır).
        self.store
            .save_messages(account_id, self.messages_snapshot(account_id))?;

        // Olay: başladı.
        self.sink.emit(StreamEvent::Started {
            account_id: account_id.into(),
            msg_id: asst_id,
            model: model.clone(),
        });

        // Arka plan görevi.
        let me = self.clone();
        let cfg = ApiConfig {
            base_url: settings.api_url.clone(),
            api_key,
            model: model.clone(),
            temperature: settings.temperature,
            max_tokens: settings.max_tokens,
            thinking_enabled: settings.thinking_enabled,
            reasoning_effort: settings.reasoning_effort.clone(),
        };
        let account_id_owned = account_id.to_string();
        let session_clone = session.clone();
        let sink = self.sink.clone();

        tokio::spawn(async move {
            me.run_stream(
                account_id_owned,
                asst_id,
                cfg,
                history,
                cancel,
                session_clone,
                sink,
            )
            .await;
        });

        Ok(asst_id)
    }

    /// Üretimi durdur.
    pub fn stop(&self, account_id: &str) {
        let map = self.sessions.lock().unwrap();
        if let Some(s) = map.get(account_id) {
            if let Some(c) = s.lock().unwrap().cancel.take() {
                c.cancel();
            }
        }
    }

    /// Yeni sohbet: mesajları temizle, devam eden isteği iptal et.
    pub fn new_chat(&self, account_id: &str) -> Result<()> {
        self.stop(account_id);
        let map = self.sessions.lock().unwrap();
        if let Some(s) = map.get(account_id) {
            let mut g = s.lock().unwrap();
            g.messages.clear();
            g.next_id = AtomicU64::new((now_millis().max(1) as u64).max(1));
        }
        drop(map);
        self.store.save_messages(account_id, Vec::new())?;
        Ok(())
    }

    /// Hesabın tercih ettiği modeli ayarla.
    pub fn set_model(&self, account_id: &str, model: Option<String>) -> Result<()> {
        self.store.set_account_model(account_id, model)
    }

    /// Hesabın tercih ettiği modeli oku.
    pub fn model(&self, account_id: &str) -> Result<String> {
        let account = self
            .store
            .accounts()
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| Error::AccountNotFound(account_id.into()))?;
        Ok(resolve_model(&account, &self.store.settings()))
    }

    /// API anahtarını doğrula.
    pub async fn test_api_key(&self, account_id: &str) -> Result<()> {
        let cfg = self.config_for(account_id)?;
        self.client.ping(&cfg).await
    }

    /// Bir hesap için geçerli [`ApiConfig`] derle.
    pub fn config_for(&self, account_id: &str) -> Result<ApiConfig> {
        let account = self
            .store
            .accounts()
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| Error::AccountNotFound(account_id.into()))?;
        let api_key = self
            .store
            .api_key(account_id)?
            .ok_or(Error::MissingApiKey)?;
        let settings = self.store.settings();
        let model = resolve_model(&account, &settings);
        Ok(ApiConfig {
            base_url: settings.api_url,
            api_key,
            model,
            temperature: settings.temperature,
            max_tokens: settings.max_tokens,
            thinking_enabled: settings.thinking_enabled,
            reasoning_effort: settings.reasoning_effort,
        })
    }

    // ---- iç ----

    /// Mesaj aynasının anlık kopyası (kalıcı yazma için).
    fn messages_snapshot(&self, account_id: &str) -> Vec<Msg> {
        let map = self.sessions.lock().unwrap();
        map.get(account_id)
            .map(|s| s.lock().unwrap().messages.clone())
            .unwrap_or_default()
    }

    /// Akış görevinin gövdesi. Belirli bir mesajı doldurur, olayları yayar.
    async fn run_stream(
        &self,
        account_id: AccountId,
        msg_id: MsgId,
        cfg: ApiConfig,
        history: Vec<ApiMessage>,
        cancel: CancellationToken,
        session: Arc<Mutex<AccountSession>>,
        sink: Arc<dyn EventSink>,
    ) {
        let mut stream = Box::pin(self.client.stream_chat(&cfg, &history, cancel));

        let mut final_text = String::new();
        let mut final_reasoning = String::new();
        let mut cancelled = false;

        while let Some(item) = stream.next().await {
            match item {
                Ok(Delta::Content(c)) => {
                    final_text.push_str(&c);
                    // Bellek aynasını güncelle.
                    {
                        let mut g = session.lock().unwrap();
                        update_pending(&mut g.messages, msg_id, |m| {
                            m.text.push_str(&c);
                        });
                    }
                    sink.emit(StreamEvent::Delta {
                        account_id: account_id.clone(),
                        msg_id,
                        content: Some(c),
                        reasoning: None,
                    });
                }
                Ok(Delta::Reasoning(r)) => {
                    final_reasoning.push_str(&r);
                    {
                        let mut g = session.lock().unwrap();
                        update_pending(&mut g.messages, msg_id, |m| {
                            m.thinking.push_str(&r);
                        });
                    }
                    sink.emit(StreamEvent::Delta {
                        account_id: account_id.clone(),
                        msg_id,
                        content: None,
                        reasoning: Some(r),
                    });
                }
                Ok(Delta::Done) => break,
                Ok(Delta::Cancelled) => {
                    cancelled = true;
                    break;
                }
                Err(e) => {
                    let msg = e.user_message();
                    {
                        let mut g = session.lock().unwrap();
                        update_pending(&mut g.messages, msg_id, |m| {
                            m.done = true;
                            m.error = Some(msg.clone());
                            if m.text.is_empty() {
                                m.text = format!("⚠ {msg}");
                            }
                        });
                    }
                    // cancel token'ı temizle ve kalıcı yaz.
                    {
                        let mut g = session.lock().unwrap();
                        g.cancel = None;
                    }
                    let snap: Vec<Msg> = session.lock().unwrap().messages.clone();
                    let _ = self.store.save_messages(&account_id, snap);
                    sink.emit(StreamEvent::Error {
                        account_id: account_id.clone(),
                        msg_id,
                        error: msg,
                    });
                    return;
                }
            }
        }

        // Akış bitti (Done veya Cancelled). Mesajı kapat.
        {
            let mut g = session.lock().unwrap();
            update_pending(&mut g.messages, msg_id, |m| {
                m.done = true;
            });
            g.cancel = None;
            let snap = g.messages.clone();
            drop(g);
            let _ = self.store.save_messages(&account_id, snap);
        }

        if cancelled {
            sink.emit(StreamEvent::Cancelled {
                account_id,
                msg_id,
                partial: final_text,
            });
        } else {
            sink.emit(StreamEvent::Done {
                account_id,
                msg_id,
                final_text,
                final_reasoning,
            });
        }
    }
}

// ---- yardımcılar ----

/// Bekleyen asistan mesajını (id eşleşen, done=false) güncelle.
fn update_pending(messages: &mut Vec<Msg>, msg_id: MsgId, f: impl FnOnce(&mut Msg)) {
    if let Some(m) = messages.iter_mut().find(|m| m.id == msg_id && m.role == MsgRole::Assistant && !m.done) {
        f(m);
    }
}

/// Hesabın seçili modelini çöz: hesap tercihi yoksa genel ayar.
fn resolve_model(account: &Account, settings: &AppSettings) -> String {
    account
        .model
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.default_model.clone())
}

/// API'ye gönderilecek mesaj dizisini kur.
///
/// * Varsa sistem promptu (hesap tercihi > genel ayar) başa eklenir.
/// * Geçmişten yalnızca `user` ve `assistant` mesajları (done=true) alınır;
///   bekleyen (done=false) asistan mesajları ve hata mesajları atlanır.
/// * Yeni kullanıcı mesajı en sona eklenir.
fn build_api_messages(
    history: &[Msg],
    new_user: &Msg,
    account: &Account,
    settings: &AppSettings,
) -> Vec<ApiMessage> {
    let mut out = Vec::with_capacity(history.len() + 2);

    let sys = account
        .system_prompt
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let s = settings.system_prompt.as_str();
            (!s.is_empty()).then_some(s)
        });
    if let Some(s) = sys {
        out.push(ApiMessage::system(s));
    }

    for m in history {
        match m.role {
            MsgRole::User => out.push(ApiMessage::user(&m.text)),
            MsgRole::Assistant if m.done && m.error.is_none() && !m.text.is_empty() => {
                out.push(ApiMessage::assistant(&m.text));
            }
            _ => {}
        }
    }

    out.push(ApiMessage::user(&new_user.text));
    out
}

/// Closure tabanlı [`EventSink`]. Testlerde ve basit kullanımda pratiktir.
pub struct FnSink<F: Fn(StreamEvent) + Send + Sync + 'static>(pub F);

impl<F: Fn(StreamEvent) + Send + Sync + 'static> EventSink for FnSink<F> {
    fn emit(&self, event: StreamEvent) {
        (self.0)(event);
    }
}

/// Olayları toplayan test yardımcısı.
#[cfg(test)]
pub struct CollectSink(pub Mutex<Vec<StreamEvent>>);

#[cfg(test)]
impl EventSink for CollectSink {
    fn emit(&self, event: StreamEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_messages_with_system() {
        let account = Account::new("a".into(), 0);
        let mut settings = AppSettings::default();
        settings.system_prompt = "Sen yardımcısın".into();
        let history = vec![
            Msg::user(1, "selam"),
            Msg { id: 2, role: MsgRole::Assistant, text: "merhaba".into(), thinking: String::new(), files: vec![], done: true, created_at: 1, model: None, error: None },
        ];
        let new = Msg::user(3, "nasılsın");
        let msgs = build_api_messages(&history, &new, &account, &settings);
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[0].content, "Sen yardımcısın");
        assert_eq!(msgs[1].role, "user");
        assert_eq!(msgs[2].role, "assistant");
        assert_eq!(msgs[3].content, "nasılsın");
    }

    #[test]
    fn build_messages_skips_pending_and_errors() {
        let account = Account::new("a".into(), 0);
        let settings = AppSettings::default();
        let history = vec![
            Msg::user(1, "selam"),
            // bekleyen (done=false) asistan — atlanmalı
            Msg::assistant_pending(2, None),
            // hatalı asistan — atlanmalı
            Msg { id: 3, role: MsgRole::Assistant, text: "⚠ hata".into(), thinking: String::new(), files: vec![], done: true, created_at: 1, model: None, error: Some("hata".into()) },
        ];
        let new = Msg::user(4, "tekrar");
        let msgs = build_api_messages(&history, &new, &account, &settings);
        // yalnızca system yoksa: user(selam), user(tekrar) → 2
        assert_eq!(msgs.len(), 2);
        assert!(msgs.iter().all(|m| m.role == "user"));
    }

    #[test]
    fn account_model_overrides_default() {
        let mut account = Account::new("a".into(), 0);
        account.model = Some("glm-4.6".into());
        let settings = AppSettings::default();
        assert_eq!(resolve_model(&account, &settings), "glm-4.6");
    }

    #[test]
    fn session_allocates_increasing_ids() {
        let s = AccountSession::new(vec![Msg::user(100, "x")]);
        let a = s.alloc_id();
        let b = s.alloc_id();
        assert!(b > a);
        assert!(a > 100);
    }

    #[tokio::test]
    async fn send_requires_api_key() {
        use std::sync::Arc;
        let dir = std::env::temp_dir().join(format!("gan-sess-{}", now_millis()));
        let store = Arc::new(Store::open(&dir).unwrap());
        let acc = store.add_account("x".into()).unwrap();
        let sink = Arc::new(CollectSink(Mutex::new(vec![]))) as Arc<dyn EventSink>;
        let sm = SessionManager::new(store, Client::new(), sink);
        // send senkron; API anahtarı olmadığı için görev başlamadan MissingApiKey döner.
        let res = sm.send(&acc.id, "merhaba");
        assert!(matches!(res, Err(Error::MissingApiKey)), "MissingApiKey bekleniyordu, gelen: {res:?}");
    }
}
