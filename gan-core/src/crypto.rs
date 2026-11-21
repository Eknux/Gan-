//! API anahtarının rest'te (disk üzerinde) şifreli saklanması.
//!
//! ## Yaklaşım ve güvenlik notu
//!
//! API anahtarları AES-256-GCM ile şifrelenir. Şifreleme anahtarı
//! ("master key"), uygulamanın veri dizininde `master.key` adlı ayrı bir
//! dosyada saklanır; dosya `0600` izinleriyle yazılır.
//!
//! Bu, *uygulama düzeyinde* bir korumadır: anahtar bir uygulamadan
//! başka bir uygulamaya sızmasını zorlaştırır, ancak aynı kullanıcının
//! süreçleri tarafından okunabilir. Mobil cihazlarda uygulama veri
//! dizini işletim sistemi tarafından uygulama sandbox'u içinde olduğundan
//! ek bir koruma sağlar.
//!
//! **Üretim için tavsiye:** Android Keystore / iOS Keychain üzerinden
//! donanım destekli saklama, platforma özel bir Tauri plugin'i ile eklenmelidir.
//! Bu mod, o plugin gelene kadar makul bir geçiş çözümüdür.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;

use crate::error::{Error, Result};

/// Master key uzunluğu (AES-256 = 32 bayt).
const KEY_LEN: usize = 32;
/// GCM nonce uzunluğu (standart 12 bayt).
const NONCE_LEN: usize = 12;

/// Şifrelenmiş API anahtarı: düz metni değil, şifreli metni saklar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Sealed {
    /// Base64 ile kodlanmış şifreli metin.
    pub ciphertext: String,
    /// Base64 ile kodlanmış nonce.
    pub nonce: String,
}

impl Sealed {
    /// Düz metin API anahtarını verilen master key ile şifrele.
    pub fn seal(plaintext: &str, master_key: &[u8]) -> Result<Self> {
        let key = ensure_key(master_key)?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| Error::Crypto(e.to_string()))?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| Error::Crypto(e.to_string()))?;
        Ok(Self {
            ciphertext: B64.encode(&ct),
            nonce: B64.encode(nonce_bytes),
        })
    }

    /// Şifreli metni çöz ve düz metni döndür.
    pub fn open(&self, master_key: &[u8]) -> Result<String> {
        let key = ensure_key(master_key)?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| Error::Crypto(e.to_string()))?;
        let ct = B64
            .decode(&self.ciphertext)
            .map_err(|e| Error::Crypto(e.to_string()))?;
        let nonce_bytes = B64
            .decode(&self.nonce)
            .map_err(|e| Error::Crypto(e.to_string()))?;
        if nonce_bytes.len() != NONCE_LEN {
            return Err(Error::Crypto("nonce uzunluğu hatalı".into()));
        }
        let nonce = Nonce::from_slice(&nonce_bytes);
        let pt = cipher
            .decrypt(nonce, ct.as_ref())
            .map_err(|_| Error::Crypto("şifre çözme başarısız (anahtar uyumsuz veya bozuk veri)".into()))?;
        String::from_utf8(pt).map_err(|e| Error::Crypto(e.to_string()))
    }
}

/// Master key üretimi.
///
/// Verilen dizinde `master.key` dosyası yoksa rastgele 32 bayt üretir ve
/// `0600` izniyle yazar; varsa okur. Bu fonksiyon idempotent'tir.
pub fn ensure_master_key(dir: &std::path::Path) -> Result<Vec<u8>> {
    use std::fs;
    let path = dir.join("master.key");
    if path.exists() {
        let bytes = fs::read(&path)?;
        if bytes.len() == KEY_LEN {
            return Ok(bytes);
        }
        // uzunluk hatalıysa yeniden üret (bozulmuş)
    }
    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    // Atomik yazma: geçici dosya + rename
    let tmp = dir.join(".master.key.tmp");
    write_secret_file(&tmp, &key)?;
    fs::rename(&tmp, &path)?;
    Ok(key.to_vec())
}

/// Gizli dosyayı `0600` izniyle yazar (Unix).
fn write_secret_file(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.flush()?;
    Ok(())
}

fn ensure_key(master_key: &[u8]) -> Result<[u8; KEY_LEN]> {
    if master_key.len() != KEY_LEN {
        return Err(Error::Crypto(format!(
            "master key {KEY_LEN} bayt olmalı, gelen {} bayt",
            master_key.len()
        )));
    }
    let mut k = [0u8; KEY_LEN];
    k.copy_from_slice(master_key);
    Ok(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let mut key = [0u8; KEY_LEN];
        rand::thread_rng().fill_bytes(&mut key);
        let sealed = Sealed::seal("sk-test-12345", &key).unwrap();
        assert_ne!(sealed.ciphertext, "sk-test-12345");
        let opened = sealed.open(&key).unwrap();
        assert_eq!(opened, "sk-test-12345");
    }

    #[test]
    fn wrong_key_fails() {
        let mut k1 = [0u8; KEY_LEN];
        let mut k2 = [0u8; KEY_LEN];
        rand::thread_rng().fill_bytes(&mut k1);
        rand::thread_rng().fill_bytes(&mut k2);
        let sealed = Sealed::seal("secret", &k1).unwrap();
        assert!(sealed.open(&k2).is_err());
    }
}
