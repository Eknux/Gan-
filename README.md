# Gan v2

**z.ai (GLM) için Rust + Tauri 2 mobil sohbet istemcisi.**

Gan, mobil cihazda z.ai ile konuşmayı tarayıcı yerine gerçek bir uygulama üzerinden
yapmak için yazıldı. Bu sürüm (v2), eski Kotlin sürümünü sıfırdan Rust ile yeniden
yazıyor ve en temel tasarım hatasını ortadan kaldırıyor: **gizli WebView ile sayfa
kazıma (DOM scraping) yerine resmi z.ai chat completions API'sini kullanır.**

---

## Neden yeniden yazıldı?

Eski Gan (Kotlin) şu işi yapıyordu: chat.z.ai sitesini gizli bir WebView'de yüklüyor,
JavaScript köprüsüyle sayfanın DOM'una mesaj yazıyor, yanıtı DOM'dan kazıyıp okuyordu.
Bu yaklaşım iki ciddi soruna yol açıyordu:

1. **Kırılganlık:** z.ai arayüzü her değiştiğinde CSS seçicileri bozuluyor, uygulama
   çalışmaz hale geliyordu. README'de bile "seçicileri DOM tanı ile yeniden ürettir"
   talimatı vardı.
2. **Ağır ve karmaşık:** WebView yönetimi, çoklu profil, captcha algılama, blob
   indirme — hepsi tek bir ViewModel'de (~243 satır) iç içe geçmişti.

v2 bunun yerine z.ai'nin resmi OpenAI-uyumlu API'sini (`https://api.z.ai/api/paas/v4`)
çağırır. Artık seçici yok, DOM kazıma yok, captcha yok. Yanıtlar doğrudan SSE akışı
olarak gelir; akış Rust içinde `reqwest` + `tokio` ile yönetilir, iptal edilebilir,
bellek güvenliği Rust'ın ownership modeliyle garanti edilir.

---

## Mimari (özet)

```
gan-v2/
├── gan-core/            # Tauri'den bağımsız saf Rust kütüphanesi
│   └── src/             #   api/ (HTTP+SSE), store, crypto, session, models
├── src-tauri/           # Tauri 2 uygulaması (gan-core'a path bağımlı)
│   ├── src/             #   lib.rs, state, sink, commands/{accounts,chat,models,settings}
│   ├── capabilities/    #   izin seti
│   ├── icons/           #   uygulama ikonları (scripts/gen_icons.py üretir)
│   └── tauri.conf.json  #   mobil + masaüstü ayarları
├── frontend/            # Vite + TypeScript (vanilla, framework yok)
│   └── src/             #   api, state, markdown, ui/{chat,accounts,settings,components}
├── scripts/             #   gen_icons.py
└── .github/workflows/   #   android.yml (APK derleme CI)
```

Üç katman, net sınır:

- **gan-core** — z.ai API istemcisi (akış + iptal), oturum yöneticisi, kalıcı JSON depo,
  AES-256-GCM ile API anahtarı şifrelemesi. Tauri'ye bağımlı değil; `cargo test` ile
  doğrulanmış (30 birim testi).
- **src-tauri** — Tauri 2 komutları; gan-core'u UI'ya bağlar. Akış olaylarını
  `gan-stream` Tauri olayları olarak frontend'e yayar.
- **frontend** — mobil öncelikli sohbet arayüzü. Markdown render (marked + DOMPurify),
  kod vurgulama (highlight.js), akışlı yanıt, hesap/model/ayar yönetimi.

Ayrıntı için [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Ön koşullar

### Ortak
- **Rust** 1.77+ (`rustup default stable`)
- **Node.js** 20+ ve npm
- **Tauri CLI 2**: `cargo install tauri-cli --version "^2"`

### Android için ek
- **Android Studio** (Android SDK + platform-tools ile)
- **Android NDK** (Tauri önerir: 27 sürümü, `sdkmanager "ndk;27"`)
- **JDK 17** (Android Gradle Plugin bunu ister)
- Ortam değişkenleri:
  ```bash
  export ANDROID_HOME="$HOME/Android/Sdk"
  export NDK_HOME="$ANDROID_HOME/ndk/27.<patch>"   # yüklü sürüme göre
  export JAVA_HOME="<jdk-17-yolu>"
  ```
- Rust Android hedefleri:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi \
                    i686-linux-android x86_64-linux-android
  ```

---

## Kurulum ve çalıştırma

> ### ⚠️ Termux (telefon üzerinde) ile derlenemez
>
> Eğer kodu Android telefonda Termux üzerinden derlemeyi denerseniz, üç komut
> başarısız olur — bunlar **projeden değil, Termux'un Rust paketinin sınırlarından**
> kaynaklanır:
>
> 1. `cargo install tauri-cli` → *"crate `std` required to be available in rlib format"*
>    hatası. Termux'un `pkg install rust` ile gelen toolchain'i, proc-macro crate'leri
>    (serde, thiserror, syn...) için gereken `std`/`core` **rlib** dosyalarını içermiyor.
>    `gan-core` bile bu yüzden telefonda derlenemez.
> 2. `rustup target add ...` → *"No command rustup"*. Termux rustup kullanmaz; Android
>    çapraz-derleme hedeflerini yerel ekleyemezsiniz.
> 3. `cargo tauri ...` → komut yok, çünkü tauri-cli (1. adım) kurulamadı.
>
> **Çözüm:** Tıpkı Gan v1'de olduğu gibi **GitHub Actions** üzerinden derleyin.
> Proje içinde `.github/workflows/android.yml` hazır: kodu GitHub'a push'layın,
> Actions bir **kurulabilir debug APK** üretip artifact olarak verir. Adımlar için
> aşağıdaki "GitHub Actions ile derleme" bölümüne bakın.
>
> Telefonda yapılabilecek tek derleme doğrulaması: `cd frontend && npx vite build`
> (frontend'i kontrol eder; Rust tarafı CI'da doğrulanır).

### GitHub Actions ile derleme (telefon geliştiricileri için)

```bash
cd gan-v2
git init && git add . && git commit -m "Gan v2: Rust + Tauri 2 rewrite"
git remote add origin <repo-adresin>
git push -u origin main
```
GitHub'da **Actions** sekmesi → "Build Android APK" iş akışı çalışır → bittiğinde
artifact'ten `gan-android-apk` indir → telefona kur. (Frontend'i değiştirip
`npx vite build` ile doğruladıktan sonra push'layabilirsin; Rust hataları CI log'unda
görünür.)

---

### Yerel derleme (masaüstü: Linux/macOS/Windows)

Aşağıdaki adımlar **gerçek bir geliştirme makinesinde** çalışır (Termux'ta değil).

### 1. Frontend bağımlılıkları
```bash
cd frontend
npm install
```

### 2. İkonlar (isteğe bağlı — repoda zaten var)
```bash
python3 scripts/gen_icons.py          # PIL gerekir; yeniden üretmek istersen
# veya bir kaynak PNG'niz varsa: cargo tauri icon path/to/icon.png
```

### 3. Android projesini ilkle
Bu adım `src-tauri/gen/android/` klasörünü üretir (bir kez yapılır):
```bash
cargo tauri android init
```

### 4. Cihazda/emülatörde çalıştır (geliştirme)
```bash
# Çalışan bir emülatör veya USB'ye bağlı cihaz olması gerekir (adb devices).
cargo tauri android dev
```
Geliştirme sırasında frontend `http://localhost:1420`'da çalışır; Tauri onu cihaza taşır.


### 5. APK derle
```bash
cargo tauri android build
# Çıktı: src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```
İmzalamak için Tauri/Gradle imzalama ayarlarını yapın (aşağıda "İmzalama").

---

## API anahtarı

Uygulama, z.ai API'sine erişmek için bir API anahtarı ister. Anahtarı şuradan alın:
**<https://z.ai/model-api>** → "API Keys" sayfası → "Create Standard API Key".

Anahtarı uygulamada girin: **Hesap ekle** (veya Ayarlar → API anahtarı → Değiştir).
Anahtar cihazda **AES-256-GCM ile şifrelenmiş** olarak saklanır (`gan-core/src/crypto.rs`).
Şifreleme anahtarı uygulamanın özel veri dizininde `master.key` dosyasındadır.

> **Güvenlik notu:** Bu *uygulama düzeyinde* bir korumadır. Üretim için donanım
> destekli saklama (Android Keystore) önerilir; bunu platforma özel bir Tauri
> plugin'i ekleyerek yapabilirsiniz. Ayrıntı: [ARCHITECTURE.md](ARCHITECTURE.md# güvenlik).

---

## Masaüstünde de çalışır

Tauri 2 çapraz platformdur; aynı kod masaüstünde de derlenir (test için pratik):
```bash
cargo tauri dev      # masaüstü penceresinde çalıştır
cargo tauri build    # masaüstü paketleri
```
Masaüstü için `webkit2gtk` (Linux) / WebView2 (Windows) / WKWebView (macOS) gerekir.

---

## İmzalama (release APK)

Debug APK doğrudan kurulabilir. Play Store için imzalı release APK/AAB gerekir.
En pratik yol: Android Studio'da **Build → Generate Signed Bundle/APK** ile bir
keystore üretin, sonra `src-tauri/gen/android/app/build.gradle.kts` içine imzalama
yapılandırmasını ekleyin. Alternatif olarak Tauri'nin `tauri.android.signingConfig`
seçeneklerini kullanın.

---

## Gan v1 (Kotlin) ile farklar

| | Gan v1 (Kotlin) | Gan v2 (Rust) |
| --- | --- | --- |
| Erişim | Gizli WebView + DOM kazıma | Resmi z.ai API (SSE) |
| Kırılganlık | Site değişince seçiciler bozulur | Yok (API kararlı) |
| Bellek güvenliği | GC'li (JVM) | Ownership (derleme zamanı) |
| Captcha | WebView'de elle çözülür | Yok (API anahtarı yeter) |
| Çoklu hesap | WebView profili (max 3 canlı) | Sınırsız (her hesabın anahtarı) |
| Platform | Yalnız Android | Android + iOS + masaüstü |
| Akış | DOM polling (300ms) | Gerçek SSE (token token) |
| Dosya üretimi | DOM'dan blob indir | (Henüz API üzerinden değil; yol haritası) |

---

## Yol haritası

- Vision modeller (`glm-5.3-flashx`) ile görsel giriş
- Dosya üretimi (z.ai API'nin `files` yanıtı) için indirme entegrasyonu
- Sohbet başlığı / arama
- iOS derleme doğrulaması
- Donanım destekli anahtar saklama (Android Keystore plugin)

---

## Katkı ve lisans

MIT. Hata raporu ve katkı için issue/PR açın.

Bu proje z.ai ile ilişkili değildir; yalnızca onun herkese açık API'sini kullanır.
z.ai kullanım şartlarını kontrol edin.
