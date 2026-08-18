//! Bounded JSONL framing for the Codex App Server.

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

use crate::core::{AppError, AppErrorKind, RecoveryAction};

/// Maximum JSON payload bytes in one JSONL frame, excluding the line ending.
pub const MAX_JSONL_BYTES: usize = 1024 * 1024;

fn codec_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::ProtocolMismatch,
        "errors.appServerProtocolMismatch",
        RecoveryAction::InstallTestedCodex,
        code,
    )
}

/// Read one bounded JSONL frame.
///
/// `Ok(None)` means a clean EOF between frames. A non-empty unterminated
/// frame is a protocol error. The raw line is never included in the returned
/// error.
pub async fn read_jsonl_message<R>(mut reader: R) -> Result<Option<Value>, AppError>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();

    loop {
        let buffer = reader
            .fill_buf()
            .await
            .map_err(|_| codec_error("APP_SERVER_READ_FAILED"))?;

        if buffer.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            if line.len() > MAX_JSONL_BYTES {
                return Err(codec_error("APP_SERVER_LINE_TOO_LARGE"));
            }
            return Err(codec_error("APP_SERVER_TRUNCATED_LINE"));
        }

        if let Some(newline_index) = buffer.iter().position(|byte| *byte == b'\n') {
            let content_end = newline_index
                .checked_sub(1)
                .filter(|index| buffer[*index] == b'\r')
                .unwrap_or(newline_index);
            let new_len = line.len() + content_end;
            if new_len > MAX_JSONL_BYTES {
                return Err(codec_error("APP_SERVER_LINE_TOO_LARGE"));
            }
            line.extend_from_slice(&buffer[..content_end]);
            reader.consume(newline_index + 1);

            let text =
                std::str::from_utf8(&line).map_err(|_| codec_error("APP_SERVER_INVALID_UTF8"))?;
            let value =
                serde_json::from_str(text).map_err(|_| codec_error("APP_SERVER_INVALID_JSON"))?;
            return Ok(Some(value));
        }

        let new_len = line.len() + buffer.len();
        if new_len > MAX_JSONL_BYTES {
            return Err(codec_error("APP_SERVER_LINE_TOO_LARGE"));
        }
        line.extend_from_slice(buffer);
        let consumed = buffer.len();
        reader.consume(consumed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn rejects_a_line_larger_than_one_mib_before_json_parse() {
        let input = vec![b'a'; MAX_JSONL_BYTES + 1];
        let error = read_jsonl_message(BufReader::new(input.as_slice()))
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "APP_SERVER_LINE_TOO_LARGE");
    }

    #[tokio::test]
    async fn accepts_exactly_one_mib_before_newline() {
        let mut input = vec![b' '; MAX_JSONL_BYTES - 2];
        input.extend_from_slice(br#"{}"#);
        input.push(b'\n');
        let value = read_jsonl_message(BufReader::new(input.as_slice()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(value, serde_json::json!({}));
    }

    #[tokio::test]
    async fn empty_eof_returns_none() {
        let value = read_jsonl_message(BufReader::new(&b""[..])).await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn unterminated_non_empty_line_is_truncated() {
        let error = read_jsonl_message(BufReader::new(br#"{"id":1}"#.as_slice()))
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "APP_SERVER_TRUNCATED_LINE");
    }

    #[tokio::test]
    async fn accepts_lf_and_crlf() {
        for input in [b"{\"ok\":true}\n".to_vec(), b"{\"ok\":true}\r\n".to_vec()] {
            let value = read_jsonl_message(BufReader::new(input.as_slice()))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(value["ok"], true);
        }
    }

    #[tokio::test]
    async fn rejects_invalid_utf8_without_raw_line_in_error() {
        let input = vec![b'{', 0xff, b'}', b'\n'];
        let error = read_jsonl_message(BufReader::new(input.as_slice()))
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "APP_SERVER_INVALID_UTF8");
        let serialized = serde_json::to_value(error).unwrap();
        assert!(serialized.get("rawLine").is_none());
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        let error = read_jsonl_message(BufReader::new(b"not-json\n".as_slice()))
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "APP_SERVER_INVALID_JSON");
    }
}
