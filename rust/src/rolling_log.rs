//! Rolling, redacted file logging for the V1 desktop shell.
//!
//! Logs are written as complete lines to
//! `%LOCALAPPDATA%\codex-barbar\logs\codex-barbar.log`, rotated at 5 MiB into
//! timestamped segments, and segments older than 14 days are deleted on
//! startup. Every complete line passes through `SecretRedactor` before it is
//! written.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing_subscriber::fmt::MakeWriter;

/// Maximum size of the active log before rotation.
pub const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
/// Segments older than this are removed on startup.
pub const SEGMENT_RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// Rotating, line-buffered redacted writer. Cheap to clone; all clones share
/// the same underlying file and rotation state.
#[derive(Clone)]
pub struct RollingLogWriter {
    dir: PathBuf,
    inner: Arc<Mutex<RollingLogInner>>,
}

struct RollingLogInner {
    file: Option<BufWriter<File>>,
    pending: Vec<u8>,
    bytes: u64,
}

impl RollingLogWriter {
    pub fn new(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        let inner = RollingLogInner::open(dir.join("codex-barbar.log"))?;
        let writer = Self {
            dir,
            inner: Arc::new(Mutex::new(inner)),
        };
        writer.cleanup_old_segments();
        Ok(writer)
    }

    fn current_path(&self) -> PathBuf {
        self.dir.join("codex-barbar.log")
    }

    fn rotate(&self, inner: &mut RollingLogInner) -> io::Result<()> {
        if let Some(file) = inner.file.as_mut() {
            file.flush()?;
        }
        // Close the active file before renaming it to a segment.
        inner.file = None;
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        let segment = self.dir.join(format!("codex-barbar.log.{stamp}"));
        fs::rename(self.current_path(), &segment)?;
        inner.file = Some(BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.current_path())?,
        ));
        inner.bytes = 0;
        Ok(())
    }

    fn cleanup_old_segments(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let cutoff = chrono::Utc::now().naive_utc()
            - chrono::Duration::from_std(SEGMENT_RETENTION).unwrap_or_default();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(stamp) = name.strip_prefix("codex-barbar.log.") else {
                continue;
            };
            let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%SZ") else {
                continue;
            };
            if parsed < cutoff {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

impl RollingLogInner {
    fn open(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            file: Some(BufWriter::new(file)),
            pending: Vec::new(),
            bytes,
        })
    }

    fn write_line(&mut self, line: &[u8], writer: &RollingLogWriter) -> io::Result<()> {
        let text = String::from_utf8_lossy(line);
        let redacted = crate::core::SecretRedactor::redact(&text);
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is closed"))?;
        file.write_all(redacted.as_bytes())?;
        file.write_all(b"\n")?;
        self.bytes += redacted.len() as u64 + 1;
        if self.bytes >= MAX_LOG_BYTES {
            writer.rotate(self)?;
        }
        Ok(())
    }
}

impl Write for RollingLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("rolling log lock poisoned"))?;
        inner.pending.extend_from_slice(buf);
        while let Some(position) = inner.pending.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = inner.pending.drain(..=position).collect();
            line.pop();
            inner.write_line(&line, self)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("rolling log lock poisoned"))?;
        if !inner.pending.is_empty() {
            let line = std::mem::take(&mut inner.pending);
            inner.write_line(&line, self)?;
        }
        if let Some(file) = inner.file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

/// `MakeWriter` bridge so tracing can hand out short-lived guards.
pub struct RollingLogMakeWriter(RollingLogWriter);

pub struct RollingLogGuard(RollingLogWriter);

impl Write for RollingLogGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a> MakeWriter<'a> for RollingLogMakeWriter {
    type Writer = RollingLogGuard;

    fn make_writer(&'a self) -> Self::Writer {
        RollingLogGuard(self.0.clone())
    }
}

/// Build the writer from canonical app paths, falling back to a
/// process-local default only when paths are unavailable.
pub fn rolling_log_writer() -> io::Result<RollingLogMakeWriter> {
    let dir = crate::app_paths::AppPaths::discover()
        .map(|paths| paths.logs)
        .unwrap_or_else(|_| PathBuf::from("codex-barbar-logs"));
    Ok(RollingLogMakeWriter(RollingLogWriter::new(dir)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, RollingLogWriter) {
        let dir = tempfile::tempdir().unwrap();
        let writer = RollingLogWriter::new(dir.path().to_path_buf()).unwrap();
        (dir, writer)
    }

    #[test]
    fn writes_redacted_complete_lines() {
        let (_dir, mut writer) = fixture();
        writer
            .write_all(b"Authorization: Bearer hunter2\n")
            .unwrap();
        writer.flush().unwrap();
        let path = writer.current_path();
        let text = fs::read_to_string(path).unwrap();
        assert!(!text.contains("hunter2"));
        assert!(text.contains("[REDACTED]"));
    }

    #[test]
    fn rotates_at_max_size() {
        let dir = tempfile::tempdir().unwrap();
        let writer = RollingLogWriter::new(dir.path().to_path_buf()).unwrap();
        {
            let mut inner = writer.inner.lock().unwrap();
            inner.bytes = MAX_LOG_BYTES - 4;
        }
        let mut writer = writer;
        writer.write_all(b"12345\n").unwrap();
        let segments = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("codex-barbar.log.")
            })
            .count();
        assert!(segments >= 1, "rotation must create a segment");
    }

    #[test]
    fn startup_removes_expired_segments() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("codex-barbar.log.20200101T000000Z");
        fs::write(&old, b"old").unwrap();
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        let new = dir.path().join(format!("codex-barbar.log.{stamp}"));
        fs::write(&new, b"new").unwrap();
        RollingLogWriter::new(dir.path().to_path_buf()).unwrap();
        assert!(!old.exists());
        assert!(new.exists());
    }
}
