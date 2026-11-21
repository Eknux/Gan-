//! z.ai HTTP istemcisi.
//!
//! Tek bir [`reqwest::Client`] paylaşır (bağlantı havuzu, HTTP/2). İki yöntem:
//!
//! * [`Client::stream_chat`] — Akışlı (SSE) sohbet. [`Delta`] akışı döndürür.
//! * [`Client::ping`] — API anahtarını doğrulamak için küçük, akışsız istek.
//!
//! İstek iptali [`tokio_util::sync::CancellationToken`] ile yapılır: kullanıcı
//! "Durdur" bastığında token tetiklenir, akış erken sonlanır ve [`Delta::Cancelled`]
//! yayılır. Bu, devam eden HTTP isteğini de `tokio::select!` ile bırakır.

use std::time::Duration;

use futures_core::Stream;
use reqwest::header::{ACCEPT, CACHE_CONTROL};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::models::ApiConfig;

use super::types::{ApiMessage, ChatRequest, Delta, StreamChunk};

/// z.ai API istemcisi. Clone ucuzdur (`Arc` tabanlı).
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
}

impl Client {
    /// Varsayılan zaman aşımlarıyla yeni istemci.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300)) // akışlı istekler için geniş
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_nodelay(true)
            .user_agent("Gan/0.2 (z.ai client)")
            .build()
            .expect("reqwest client build");
        Self { http }
    }

    /// Belirli bir `reqwest::Client` kullan (test/proxy için).
    pub fn with_http(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// API anahtarını küçük bir istekle doğrula. 200 dönerse `Ok(())`.
    ///
    /// Maliyeti düşük tutmak için `max_tokens: 1` ile tek-kelimelik bir
    /// tamamlama isteği atar; akış kapalıdır.
    pub async fn ping(&self, cfg: &ApiConfig) -> Result<()> {
        let mut msg = ApiMessage::user("1");
        let _ = &mut msg; // (gelecekte içerik genişletilebilir)
        let mut body = ChatRequest::new(cfg, std::slice::from_ref(&msg), false);
        body.max_tokens = Some(1);
        body.thinking = None;
        body.reasoning_effort = None;

        let resp = self
            .http
            .post(cfg.chat_url())
            .bearer_auth(&cfg.api_key)
            .header(ACCEPT, "application/json")
            .json(&body)
            .send()
            .await
            .map_err(Error::Network)?;

        let status = resp.status();
        if status.is_success() {
            let _ = resp.bytes().await; // gövdeyi tüket, bağlantıyı havuza geri ver
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(Error::ApiHttp {
                status: status.as_u16(),
                body,
            })
        }
    }

    /// Akışlı sohbet. Döndürülen akış `Result<Delta>` öğeleri yayarak ilerler;
    /// `Ok(Delta::Done)` veya `Ok(Delta::Cancelled)` geldikten sonra akış kapanır.
    ///
    /// `cancel` tetiklendiğinde devam eden HTTP isteği bırakılır ve akış
    /// `Ok(Delta::Cancelled)` ile sonlanır.
    pub fn stream_chat(
        &self,
        cfg: &ApiConfig,
        messages: &[ApiMessage],
        cancel: CancellationToken,
    ) -> impl Stream<Item = Result<Delta>> {
        let http = self.http.clone();
        let cfg = cfg.clone();
        let messages = messages.to_vec();

        async_stream::stream! {
            if cancel.is_cancelled() {
                yield Ok(Delta::Cancelled);
                return;
            }

            let body = ChatRequest::new(&cfg, &messages, true);
            let req = http
                .post(cfg.chat_url())
                .bearer_auth(&cfg.api_key)
                .header(ACCEPT, "text/event-stream")
                .header(CACHE_CONTROL, "no-cache")
                .json(&body);

            // HTTP isteğini gönder; aynı anda cancel'a da kulak ver.
            let resp = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    yield Ok(Delta::Cancelled);
                    return;
                }
                r = req.send() => match r {
                    Ok(r) => r,
                    Err(e) => { yield Err(Error::Network(e)); return; }
                },
            };

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                yield Err(Error::ApiHttp { status: status.as_u16(), body });
                return;
            }

            let mut byte_stream = resp.bytes_stream();
            let mut buf: Vec<u8> = Vec::with_capacity(8192);

            loop {
                // Bir sonraki bayt yığını — ya cancel ya da veri.
                let chunk = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        yield Ok(Delta::Cancelled);
                        return;
                    }
                    c = byte_stream.next() => c,
                };

                let bytes = match chunk {
                    Some(Ok(b)) => b,
                    Some(Err(e)) => { yield Err(Error::Network(e)); return; }
                    None => break, // akış kapandı
                };
                buf.extend_from_slice(&bytes);

                // Tam satırları işle. SSE: satır `\n` ile biter; her `data:`
                // satırı kendi başına bir olay parçasıdır (z.ai tek satırda
                // bir data yazar). Olaylar boş satırla ayrılır; biz her satırı
                // bağımsız ele alırız.
                loop {
                    let nl = match buf.iter().position(|&b| b == b'\n') {
                        Some(p) => p,
                        None => break,
                    };
                    let line_bytes: Vec<u8> = buf.drain(..=nl).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let line = line.trim_end_matches('\r').trim();
                    if line.is_empty() {
                        continue;
                    }
                    // SSE yorum satırı (keepalive) `:` ile başlar.
                    if line.starts_with(':') {
                        continue;
                    }
                    let data = match line.strip_prefix("data:") {
                        Some(d) => d.trim(),
                        None => continue, // event:/id:/retry: yok say
                    };
                    if data == "[DONE]" {
                        yield Ok(Delta::Done);
                        return;
                    }
                    if data.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<StreamChunk>(data) {
                        Ok(chunk) => {
                            if let Some(d) = chunk.delta() {
                                yield Ok(d);
                            }
                            if chunk.is_finished() {
                                yield Ok(Delta::Done);
                                return;
                            }
                        }
                        Err(_) => {
                            // Tanımadığımız ara satır; atla, akışı bozma.
                            continue;
                        }
                    }
                }
            }

            // Akış `[DONE]` göndermeden kapandıysa yine de tamam say:
            // buraya ulaşılması, döngünün veri tükenip bittiği anlamına gelir
            // (z.ai bazen son `[DONE]` satırını göndermeden kapanabilir).
            yield Ok(Delta::Done);
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ApiConfig;

    fn cfg() -> ApiConfig {
        ApiConfig {
            base_url: "https://api.z.ai/api/paas/v4".into(),
            api_key: "fake".into(),
            model: "glm-5.2".into(),
            temperature: 1.0,
            max_tokens: None,
            thinking_enabled: true,
            reasoning_effort: "high".into(),
        }
    }

    #[test]
    fn client_clone_is_cheap() {
        let c = Client::new();
        let _c2 = c.clone();
    }

    #[tokio::test]
    async fn ping_returns_err_for_fake_key() {
        // Gerçek ağ isteği; sahte anahtarla 401/403 beklenir. Ağ yoksa
        // `Network` hatası döner — her ikisi de testi geçer sayılır.
        let c = Client::new();
        let res = c.ping(&cfg()).await;
        assert!(res.is_err(), "sahte anahtarla hata beklenir");
    }
}
