# Gan v2 — Mimari

Bu belge, üç katmanın (çekirdek, Tauri uygulaması, frontend) nasıl etkileştiğini,
veri akışını, akış (streaming) boru hattını ve güvenlik modelini açıklar.

## Katman diyagramı

```
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (TypeScript)                   │
│   chat.ts ─── api.ts ──→ invoke("send_message", …)          │
│      ↑                      │                                │
│      │ gan-stream olayı ←── listen ─────────────────┐        │
└──────┼──────────────────────────────────────────────┼────────┘
       │                                              │
       │  Tauri IPC                                   │ Tauri olayları
       ▼                                              ▼
┌──────────────────────────────────────────────────────────────┐
│                   src-tauri (Tauri 2 uygulaması)              │
│   commands/{chat,accounts,models,settings}.rs                 │
│      │  State<AppState>                                       │
│      ▼                                                        │
│   AppState { store: Arc<Store>, sessions: SessionManager }    │
│      │                                                        │
│      ▼  sink.emit(StreamEvent) → app_handle.emit("gan-stream")│
└──────┼────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│                      gan-core (saf Rust)                      │
│   SessionManager ──→ Client.stream_chat(cfg, msgs, cancel)    │
│        │                        │                              │
│        ▼                        ▼                              │
│   Store (gan.json)         reqwest + tokio → api.z.ai SSE      │
│   crypto (AES-256-GCM)     Delta::{Content, Reasoning, Done}   │
└──────────────────────────────────────────────────────────────┘
```

## gan-core: çekirdek kütüphane

Tauri'ye (veya herhangi bir UI çerçevesine) bağımlı değildir. Tek başına
`cargo test` ile doğrulanır. Beş modül:

### `models.rs`
Veri tipleri: `Account`, `Msg`, `MsgRole`, `FileRef`, `AppSettings`, `ApiConfig`,
`Theme`. Sabitler: `DEFAULT_API_URL`, `DEFAULT_MODEL`, `SUPPORTED_MODELS`,
`AVATAR_PALETTE`. Tüm tipler `serde` ile serileştirilir; bu sayede Tauri komutları
doğrudan bu tipleri frontend'e geçirebilir.

### `store.rs` — `Store`
Tek bir `gan.json` dosyasına okur/yazar:
- Hesaplar, hesap başına mesaj geçmişi (son 300), ayarlar, şifreli API anahtarları.
- Yazma **atomik**: geçici dosya `.gan.json.tmp`'ye yazılır, sonra `rename`. Yarım
  yazma veri kaybına yol açmaz.
- Bozuk JSON durumunda dosya `.bak`'a yedeklenir ve sıfırlanır; uygulama çökmez.
- Şema sürümü (`version: 1`); `migrate()` gelecekteki göçler için kanal.

### `crypto.rs` — `Sealed`
API anahtarları **AES-256-GCM** ile şifrelenir:
- Master key: 32 bayt rastgele, `master.key` dosyasında `0600` izniyle.
- `Sealed::seal(plaintext, master_key)` → `{ciphertext, nonce}` (base64).
- `Sealed::open` çözüm; yanlış anahtarda hata döner (GCM etiket doğrulaması).
- İlk çalıştırmada `ensure_master_key()` master key üretir (idempotent).

### `api/` — z.ai HTTP istemcisi
- `types.rs`: `ChatRequest` (serileştirme), `StreamChunk` (çözümleme — yalnızca
  `choices[].delta.{content,reasoning_content}` ve `finish_reason` çözülür; geri
  kalan alan z.ai yeni şey eklediğinde kırılmaz).
- `client.rs`: `Client` tek bir `reqwest::Client` paylaşır (HTTP/2, bağlantı havuzu).
  - `ping(cfg)`: küçük akışsız istekle anahtar doğrulama.
  - `stream_chat(cfg, messages, cancel)`: `impl Stream<Item = Result<Delta>>`.
    `async_stream::stream!` ile üretilir. `tokio::select!` ile hem HTTP isteğini hem
    de `CancellationToken`'ı aynı anda bekler; kullanıcı "Durdur" basınca istek
    bırakılır ve `Delta::Cancelled` yayılır.
- `stream.rs`: SSE satır çözümleyici — `data:` satırlarını ayıklar, `[DONE]` ve
  `finish_reason`'ı tanır. `extract_lines`/`parse_data_line` test edilebilir birimler.

### `session.rs` — `SessionManager`
Hesap başına canlı oturum durumu (`HashMap<AccountId, Arc<Mutex<AccountSession>>>`):
- Mesajların bellekte aynası (UI için hızlı okuma).
- Devam eden istek için `CancellationToken`.
- Monoton mesaj kimliği (`AtomicU64`).

`send(account_id, text)` senkron ve **hemen döner**:
1. Kullanıcı + bekleyen asistan mesajını belleğe ekler, kullanıcı mesajını diske yazar.
2. `ApiConfig`'ı derler (hesap modeli > genel ayar; API anahtarı çözülür).
3. `build_api_messages` ile API'ye gönderilecek diziyi kurur (sistem promptu başa,
   tamamlanmış user/assistant geçmişi, yeni kullanıcı mesajı sona; bekleyen ve
   hatalı mesajlar atlanır).
4. Bir `tokio::spawn` görevi başlatır: akışı tüketir, her `Delta`'da bellek aynasını
   günceller ve `EventSink::emit(StreamEvent)` ile UI'ya bildirir.

`StreamEvent` varyantları (serde `tag = "kind"`): `started`, `delta`, `done`,
`cancelled`, `error`.

## src-tauri: Tauri 2 uygulaması

### Giriş (`lib.rs`)
`#[cfg_attr(mobile, tauri::mobile_entry_point)] pub fn run()` — masaüstü ve mobil
için tek giriş. `setup` kancasında:
1. `app.path().app_data_dir()` ile veri dizinini çöz (Android'de uygulama iç dizini).
2. `Store::open`, `Client::new`, `TauriSink` (AppHandle saran `EventSink`), `SessionManager`
   kur; `AppState`'i `app.manage()` ile kaydet.

### Komutlar (`commands/`)
Her komut `#[tauri::command]` ile işaretli, `State<'_, AppState>` alır, hataları
`Error::user_message()` ile Türkçe dizeye eşler. Gruplar:
- **accounts**: `list/add/remove/select/active_account`, `set_account_model`,
  `set_account_system_prompt`, `set_api_key`, `has_api_key`.
- **chat**: `send_message` (asistan msg kimliği döner), `stop`, `new_chat`,
  `list_messages`, `is_generating`.
- **models**: `supported_models` (statik liste), `account_model`.
- **settings**: `get_settings`, `save_settings`, `test_api_key` (async).

### Olay yayını (`sink.rs`)
`TauriSink` `EventSink` uygular: `emit(event)` → `app_handle.emit("gan-stream", &event)`.
Frontend `listen("gan-stream", …)` ile dinler.

### İzinler (`capabilities/default.json`)
`core:default`, `event`, `window`, `app`, `path` ve `opener` izinleri. Dışarıya HTTP
isteği Rust içinde (`reqwest`) yapıldığı için bir HTTP plugin gerekmez; JS `fetch`
kullanılmaz (CSP `connect-src` yalnızca z.ai'ye izin verir, ama bu yalnızca güvenlik
katmanı — asıl istekler Rust'tan).

## Frontend: Vite + TypeScript

Vanilla TypeScript, framework yok. Tek bir reaktif durum kaynağı (`state.ts`):
`getState/setState/subscribe`. UI, abonelerle state değişince yeniden çizer.

### Akış güncelleme stratejisi (verimlilik)
`main.ts` `handleStreamEvent` ile her olayı işler:
- `started`: bekleyen asistan mesajını `state.messages`'a ekler, `renderChat`'ın
  `onStreamStarted`'ını çağırır (tek DOM düğümü ekler — tüm liste yeniden çizilmez).
- `delta`: `state.messages`'daki ilgili mesajın metnine parçayı ekler, sonra
  `onStreamDelta(msgId)` yalnızca o baloncuğun `.streaming-text` span'ını günceller
  (ham metin, **markdown yeniden parse edilmez**).
- `done`/`cancelled`/`error`: mesajı kapatır, `onStreamFinalize(msgId)` ile o
  baloncuğu **tam markdown** ile yeniden çizer ve kod bloklarını vurgular.

Bu, her token'da tüm listeyi yeniden render etmek yerine yalnızca akış halindeki
baloncuğu günceller — uzun yanıtlarda akıcı kalır.

### Markdown (`markdown.ts`)
- `marked` ayrıştırır, `DOMPurify` XSS'e karşı temizler (model zararlı HTML döndürebilir).
- `highlightCodeBlocks` DOM'a yerleştikten sonra `pre > code` bloklarını
  `hljs.highlightElement` ile vurgular; her bloğa "Kopyala" düğmesi ekler.
- Yalnızca yaygın diller yüklenir (bundle ~56KB gzip).

## Güvenlik

### API anahtarı saklama
- AES-256-GCM, master key `master.key` dosyasında (`0600`).
- Mobilde uygulama veri dizini OS sandbox'u içinde → başka uygulamalar erişemez.
- **Sınırlama:** aynı kullanıcının süreçleri okuyabilir. Üretim için Android
  Keystore / iOS Keychain (donanım destekli) bir Tauri plugin'i ile eklenmeli.
  Bu, `crypto.rs`'nin `Sealed` tipini değiştirmeden — yalnızca master key kaynağını
  değiştirerek — yapılabilir: `ensure_master_key` yerine Keystore'dan oku.

### CSP
`tauri.conf.json` içinde `connect-src 'self' https://api.z.ai https://*.z.ai`.
JS `fetch` kullanılmadığı için bu yalnızca savunma derinliği; asıl istekler Rust'tan.

### Girdi doğrulama
- `save_settings`: sıcaklık 0–1, model boş değil.
- `set_api_key`: boş reddedilir, trim'lenir.
- `send`: boş metaj reddedilir, aynı anda yalnız bir üretim.

## Test kapsamı (gan-core)

`cargo test` — 30 birim testi:
- `models`: kimlik üretimi, ayar varsayılanları, serileştirme gidiş-dönüşü.
- `crypto`: şifrele/çöz gidiş-dönüşü, yanlış anahtar başarısız.
- `store`: açma, hesap ekleme (palet), API anahtarı gidiş-dönüşü, mesaj kırpma
  (300 sınır), silme temizliği.
- `api/types`: istek gövdesi serileştirme, chunk çözümleme (content/reasoning/finish).
- `api/stream`: SSE satır ayıklama, `[DONE]`, yorum satırı, bozuk JSON.
- `api/client`: clone ucuz, sahte anahtarla ping hatası (gerçek ağ).
- `session`: sistem promptu ekleme, bekleyen/hatalı mesaj atlama, model önceliği,
  kimlik artışı, API anahtarı zorunluluğu.

Tauri uygulaması ve frontend bu sandbox'ta tam derlenemiyor (Tauri sistem
webkit bağımlılıkları + Android SDK yok), ancak:
- gan-core `cargo check --all-targets` ile temiz.
- Frontend `tsc --noEmit` ve `vite build` ile temiz (168KB bundle, 56KB gzip).
