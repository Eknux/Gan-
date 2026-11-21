//! Kalıcı yerel depo.
//!
//! Hesapları, hesap başına mesaj geçmişini, ayarları ve şifreli API
//! anahtarlarını tek bir JSON dosyasında (`gan.json`) tutar. Yazmalar
//! geçici dosya + `rename` ile atomiktir, böylece yarım yazma veri
//! kaybına yol açmaz. Okumalar hataya dayanıklıdır: bozuk bir alan
//! atılır, uygulama çökmez.
//!
//! ## Dosya düzeni
//!
//! ```json
//! {
//!   "version": 1,
//!   "settings": { ... },
//!   "accounts": [ { "id": "...", "name": "...", ... } ],
//!   "messages": { "<account_id>": [ { "id": 1, "role": "user", ... } ] },
//!   "keys":     { "<account_id>": { "ciphertext": "...", "nonce": "..." } },
//!   "active":   "<account_id>"
//! }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::crypto::{ensure_master_key, Sealed};
use crate::error::Result;
use crate::models::{Account, AccountId, AppSettings, Msg, MsgRole, AVATAR_PALETTE};

/// Depo dosya biçimi sürümü. İleride göç (migration) gerektiğinde artırılır.
const SCHEMA_VERSION: u32 = 1;
const MAX_MSGS_PER_ACCOUNT: usize = 300;

/// Disk üzerindeki depo yapısı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreData {
    pub version: u32,
    #[serde(default)]
    pub settings: AppSettings,
    #[serde(default)]
    pub accounts: Vec<Account>,
    /// account_id → mesaj listesi.
    #[serde(default)]
    pub messages: HashMap<AccountId, Vec<Msg>>,
    /// account_id → şifreli API anahtarı.
    #[serde(default)]
    pub keys: HashMap<AccountId, Sealed>,
    /// Aktif hesap kimliği.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<AccountId>,
}

impl Default for StoreData {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            settings: AppSettings::default(),
            accounts: Vec::new(),
            messages: HashMap::new(),
            keys: HashMap::new(),
            active: None,
        }
    }
}

/// Kalıcı depo. Tek bir dosyaya okur/yazar; master key ayrı dosyada.
pub struct Store {
    path: PathBuf,
    data_dir: PathBuf,
    data: std::sync::Mutex<StoreData>,
    master_key: Vec<u8>,
}

impl Store {
    /// Yeni depo aç. Dosya yoksa varsayılanla başlatır.
    /// `data_dir` hem `gan.json` hem `master.key` için kullanılır.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;
        let path = data_dir.join("gan.json");
        let master_key = ensure_master_key(&data_dir)?;

        let data = if path.exists() {
            let raw = std::fs::read(&path)?;
            match serde_json::from_slice::<StoreData>(&raw) {
                Ok(d) => migrate(d),
                Err(e) => {
                    // Bozuk JSON: yedekle, sıfırla.
                    let bak = data_dir.join("gan.json.bak");
                    let _ = std::fs::rename(&path, &bak);
                    eprintln!("WARN: gan.json bozuk ({e}), yedeklendi ve sıfırlandı");
                    StoreData::default()
                }
            }
        } else {
            StoreData::default()
        };

        Ok(Self {
            path,
            data_dir,
            data: std::sync::Mutex::new(data),
            master_key,
        })
    }

    /// Ayarları oku (kopya).
    pub fn settings(&self) -> AppSettings {
        self.data.lock().unwrap().settings.clone()
    }

    /// Ayarları kaydet ve diske yaz.
    pub fn save_settings(&self, s: AppSettings) -> Result<()> {
        let mut g = self.data.lock().unwrap();
        g.settings = s;
        self.persist_locked(&mut g)
    }

    /// Tüm hesapları oku (kopya).
    pub fn accounts(&self) -> Vec<Account> {
        self.data.lock().unwrap().accounts.clone()
    }

    /// Aktif hesap kimliği.
    pub fn active_id(&self) -> Option<AccountId> {
        self.data.lock().unwrap().active.clone()
    }

    /// Aktif hesabı ayarla (None = hiçbiri).
    pub fn set_active(&self, id: Option<AccountId>) -> Result<()> {
        let mut g = self.data.lock().unwrap();
        g.active = id;
        self.persist_locked(&mut g)
    }

    /// Yeni hesap ekle. Döndürülen hesap artık listededir.
    pub fn add_account(&self, name: String) -> Result<Account> {
        let mut g = self.data.lock().unwrap();
        let idx = g.accounts.len();
        let color = AVATAR_PALETTE[idx % AVATAR_PALETTE.len()];
        let acc = Account::new(name, color);
        g.accounts.push(acc.clone());
        g.active = Some(acc.id.clone());
        self.persist_locked(&mut g)?;
        Ok(acc)
    }

    /// Hesabı (ve mesajlarını, anahtarını) sil.
    pub fn remove_account(&self, id: &str) -> Result<()> {
        let mut g = self.data.lock().unwrap();
        g.accounts.retain(|a| a.id != id);
        g.messages.remove(id);
        g.keys.remove(id);
        if g.active.as_deref() == Some(id) {
            g.active = g.accounts.first().map(|a| a.id.clone());
        }
        self.persist_locked(&mut g)
    }

    /// Hesabın tercih ettiği modeli güncelle.
    pub fn set_account_model(&self, id: &str, model: Option<String>) -> Result<()> {
        let mut g = self.data.lock().unwrap();
        if let Some(a) = g.accounts.iter_mut().find(|a| a.id == id) {
            a.model = model;
            self.persist_locked(&mut g)?;
        }
        Ok(())
    }

    /// Hesabın sistem promptunu güncelle.
    pub fn set_account_system_prompt(&self, id: &str, sys: Option<String>) -> Result<()> {
        let mut g = self.data.lock().unwrap();
        if let Some(a) = g.accounts.iter_mut().find(|a| a.id == id) {
            a.system_prompt = sys;
            self.persist_locked(&mut g)?;
        }
        Ok(())
    }

    /// Bir hesabın API anahtarını (düz metin) kaydet — şifreleyip saklar.
    pub fn set_api_key(&self, id: &str, plaintext: &str) -> Result<()> {
        let sealed = Sealed::seal(plaintext, &self.master_key)?;
        let mut g = self.data.lock().unwrap();
        g.keys.insert(id.to_string(), sealed);
        self.persist_locked(&mut g)
    }

    /// Bir hesabın API anahtarını çöz ve döndür. Yoksa `None`.
    pub fn api_key(&self, id: &str) -> Result<Option<String>> {
        let g = self.data.lock().unwrap();
        match g.keys.get(id) {
            Some(sealed) => {
                let s = sealed.clone();
                drop(g);
                Ok(Some(s.open(&self.master_key)?))
            }
            None => Ok(None),
        }
    }

    /// Bir hesabın API anahtarı tanımlı mı?
    pub fn has_api_key(&self, id: &str) -> bool {
        self.data.lock().unwrap().keys.contains_key(id)
    }

    /// Bir hesabın mesajlarını oku (kopya).
    pub fn messages(&self, id: &str) -> Vec<Msg> {
        self.data
            .lock()
            .unwrap()
            .messages
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// Bir hesabın tüm mesaj geçmişini değiştir ve kaydet.
    pub fn save_messages(&self, id: &str, msgs: Vec<Msg>) -> Result<()> {
        let mut g = self.data.lock().unwrap();
        let trimmed: Vec<Msg> = if msgs.len() > MAX_MSGS_PER_ACCOUNT {
            // En eski mesajları at, son N'yi tut.
            msgs.into_iter()
                .rev()
                .take(MAX_MSGS_PER_ACCOUNT)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        } else {
            msgs
        };
        g.messages.insert(id.to_string(), trimmed);
        self.persist_locked(&mut g)
    }

    /// Bir hesabın son asistan mesajını güncelle (akış sırasında çağrılır).
    /// Bu, atomik diske yazma yapmaz — yalnızca bellek durumunu günceller.
    /// Akış bittiğinde [`Store::save_messages`] çağrılmalıdır.
    pub fn update_last_assistant_in_memory(&self, id: &str, f: impl FnOnce(&mut Msg)) {
        let mut g = self.data.lock().unwrap();
        if let Some(list) = g.messages.get_mut(id) {
            if let Some(last) = list.last_mut() {
                if last.role == MsgRole::Assistant && !last.done {
                    f(last);
                }
            }
        }
    }

    /// Tüm depoyu JSON dizesi olarak dışa aktar (yedekleme/ayıklama).
    pub fn export_json(&self) -> Result<String> {
        let g = self.data.lock().unwrap();
        Ok(serde_json::to_string_pretty(&*g)?)
    }

    /// Master key (test/diagnostik için).
    pub fn master_key(&self) -> &[u8] {
        &self.master_key
    }

    /// Veri dizini.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Atomik yazma: geçici dosyaya yaz, sonra `rename`.
    fn persist_locked(&self, data: &mut StoreData) -> Result<()> {
        data.version = SCHEMA_VERSION;
        let tmp = self.data_dir.join(".gan.json.tmp");
        let bytes = serde_json::to_vec_pretty(data)?;
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// Şema göçü. Yeni sürümler eklendikçe buraya adım adım göçler eklenir;
/// şimdilik sürüm 1 tek biçim olduğundan gövde no-op'tur.
fn migrate(mut d: StoreData) -> StoreData {
    if d.version > SCHEMA_VERSION {
        // gelecekteki sürüm — bilinen en düşüğe geri indir
        d.version = SCHEMA_VERSION;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::now_millis;

    fn tmp_store() -> Store {
        let dir = std::env::temp_dir().join(format!("gan-test-{}", now_millis()));
        Store::open(&dir).unwrap()
    }

    #[test]
    fn open_creates_defaults() {
        let s = tmp_store();
        assert!(s.accounts().is_empty());
        assert_eq!(s.settings().api_url, crate::models::DEFAULT_API_URL);
    }

    #[test]
    fn add_account_assigns_palette_color() {
        let s = tmp_store();
        let a = s.add_account("bir".into()).unwrap();
        assert_eq!(a.color, AVATAR_PALETTE[0]);
        let b = s.add_account("iki".into()).unwrap();
        assert_eq!(b.color, AVATAR_PALETTE[1]);
        assert_eq!(s.active_id(), Some(b.id.clone()));
    }

    #[test]
    fn api_key_roundtrip() {
        let s = tmp_store();
        let a = s.add_account("x".into()).unwrap();
        assert!(!s.has_api_key(&a.id));
        s.set_api_key(&a.id, "sk-secret").unwrap();
        assert!(s.has_api_key(&a.id));
        assert_eq!(s.api_key(&a.id).unwrap(), Some("sk-secret".to_string()));
    }

    #[test]
    fn messages_persist_and_trim() {
        let s = tmp_store();
        let a = s.add_account("x".into()).unwrap();
        let big: Vec<Msg> = (0..(MAX_MSGS_PER_ACCOUNT + 50))
            .map(|i| Msg::user(i as u64, format!("m{i}")))
            .collect();
        s.save_messages(&a.id, big).unwrap();
        let stored = s.messages(&a.id);
        assert_eq!(stored.len(), MAX_MSGS_PER_ACCOUNT);
        // en yenisi korunmuş
        assert_eq!(stored.last().unwrap().text, format!("m{}", MAX_MSGS_PER_ACCOUNT + 49));
    }

    #[test]
    fn remove_account_cleans_keys_and_msgs() {
        let s = tmp_store();
        let a = s.add_account("x".into()).unwrap();
        s.set_api_key(&a.id, "k").unwrap();
        s.save_messages(&a.id, vec![Msg::user(1, "hi")]).unwrap();
        s.remove_account(&a.id).unwrap();
        assert!(s.accounts().iter().all(|x| x.id != a.id));
        assert!(!s.has_api_key(&a.id));
        assert!(s.messages(&a.id).is_empty());
    }
}
