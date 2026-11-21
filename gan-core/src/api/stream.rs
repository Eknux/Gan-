//! SSE (Server-Sent Events) akış çözümleyici.
//!
//! Bu mod, [`crate::api::client::Client::stream_chat`] içinde kullanılan
//! satır-temelli SSE ayrıştırma mantığını ayrı bir Test edilebilir birim
//! olarak da sunar. Üretimde istemci doğrudan `bytes_stream` üzerinde
//! ayrıştırma yapar; buradaki [`parse_lines`] ise bir bayt tamponundan
//! tam satırları ayıklar ve testlerde kullanılır.
//!
//! ## SSE biçimi (z.ai için)
//!
//! ```text
//! data: {"choices":[{"delta":{"content":"mer"}}]}
//!
//! data: {"choices":[{"delta":{"content":"haba"}}]}
//!
//! data: [DONE]
//! ```
//!
//! * Her olay `data:` önekli bir satırdır.
//! * Olaylar boş satırla (`\n\n`) ayrılır.
//! * `:` ile başlayan satırlar yorum/keepalive'dir, atlanır.
//! * `data: [DONE]` akışı kapatır.

use crate::api::types::{Delta, StreamChunk};
use crate::error::{Error, Result};

/// Bir tampondan tam satırları ayıkla. `\n` görene kadar tamponda tutar;
/// tamamlanan satırları (CR/LF temizlenmiş) döndürür. Dönen vektör
/// tamponun *başından* tüketilmiş parçayı ifade eder; kalan baytlar
/// tamponda bırakılır.
///
/// `drained` = tampondan çıkarılan toplam bayt sayısı (çağıran bunu
/// `buf.drain(..drained)` için kullanır).
pub fn extract_lines(buf: &[u8]) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut i = 0;
    let mut consumed = 0;
    while i < buf.len() {
        if buf[i] == b'\n' {
            let slice = &buf[consumed..i];
            let line = String::from_utf8_lossy(slice)
                .trim_end_matches('\r')
                .trim()
                .to_string();
            lines.push(line);
            consumed = i + 1;
        }
        i += 1;
    }
    (lines, consumed)
}

/// Bir `data:` satırından [`Delta`] çıkar. Yorum/boş/event satırlar `None`.
/// `[DONE]` → `Some(Delta::Done)`.
pub fn parse_data_line(line: &str) -> Option<Result<Delta>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return None;
    }
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() {
        return None;
    }
    if data == "[DONE]" {
        return Some(Ok(Delta::Done));
    }
    match serde_json::from_str::<StreamChunk>(data) {
        Ok(chunk) => {
            if chunk.is_finished() {
                Some(Ok(Delta::Done))
            } else {
                chunk.delta().map(Ok)
            }
        }
        Err(e) => Some(Err(Error::StreamJson(e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_lines_handles_partial() {
        let buf = b"data: hello\ndata: wor";
        let (lines, consumed) = extract_lines(buf);
        assert_eq!(lines, vec!["data: hello".to_string()]);
        assert_eq!(consumed, 12); // "data: hello\n"
        assert_eq!(&buf[consumed..], b"data: wor");
    }

    #[test]
    fn extract_lines_handles_crlf() {
        let buf = b"data: a\r\ndata: b\r\n";
        let (lines, _) = extract_lines(buf);
        assert_eq!(lines, vec!["data: a".to_string(), "data: b".to_string()]);
    }

    #[test]
    fn parse_done_sentinel() {
        assert!(matches!(parse_data_line("data: [DONE]"), Some(Ok(Delta::Done))));
    }

    #[test]
    fn parse_content() {
        let line = r#"data: {"choices":[{"delta":{"content":"x"},"finish_reason":null}]}"#;
        assert!(matches!(parse_data_line(line), Some(Ok(Delta::Content(_)))));
    }

    #[test]
    fn parse_reasoning() {
        let line = r#"data: {"choices":[{"delta":{"reasoning_content":"düşün"}}]}"#;
        assert!(matches!(parse_data_line(line), Some(Ok(Delta::Reasoning(_)))));
    }

    #[test]
    fn parse_finish_reason() {
        let line = r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        assert!(matches!(parse_data_line(line), Some(Ok(Delta::Done))));
    }

    #[test]
    fn parse_comment_and_empty_return_none() {
        assert!(parse_data_line(": keepalive").is_none());
        assert!(parse_data_line("").is_none());
        assert!(parse_data_line("event: ping").is_none()); // data: değil
    }

    #[test]
    fn parse_bad_json_returns_err() {
        let line = "data: {böke bir şey}";
        assert!(matches!(parse_data_line(line), Some(Err(_))));
    }
}
