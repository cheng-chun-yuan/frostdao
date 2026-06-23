//! Append-only operational audit log helpers.
//!
//! Audit events are intentionally metadata-only. Do not log mnemonics,
//! secret shares, nonces, signature shares, ciphertext payloads, or raw
//! transactions here.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

const DEFAULT_AUDIT_LOG: &str = ".frost_state/audit.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub timestamp_secs: u64,
    pub event: String,
    pub wallet: String,
    pub status: String,
    #[serde(skip_serializing_if = "Map::is_empty", default)]
    pub fields: Map<String, Value>,
}

impl AuditEvent {
    pub fn new(
        event: impl Into<String>,
        wallet: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            timestamp_secs: current_timestamp_secs(),
            event: event.into(),
            wallet: wallet.into(),
            status: status.into(),
            fields: Map::new(),
        }
    }

    pub fn with_field(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        self.fields.insert(key.into(), value);
        self
    }
}

pub fn append(event: &AuditEvent) -> Result<()> {
    append_to(default_audit_path(), event)
}

pub fn append_to(path: impl AsRef<Path>, event: &AuditEvent) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn default_audit_path() -> PathBuf {
    std::env::var_os("FROSTDAO_AUDIT_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_AUDIT_LOG))
}

fn current_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{append_to, AuditEvent};
    use serde_json::Value;

    #[test]
    fn audit_event_appends_jsonl_without_secret_fields() {
        let path =
            std::env::temp_dir().join(format!("frostdao-audit-test-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let event = AuditEvent::new("dkg_build_tx", "treasury", "prepared")
            .with_field("session_id", "abc123")
            .with_field("amount_sats", 10_000u64)
            .with_field("sighash_fingerprint", "001122...aabbcc");

        append_to(&path, &event).unwrap();

        let data = std::fs::read_to_string(&path).unwrap();
        let line = data.lines().next().unwrap();
        let parsed: Value = serde_json::from_str(line).unwrap();

        assert_eq!(parsed["event"], "dkg_build_tx");
        assert_eq!(parsed["wallet"], "treasury");
        assert_eq!(parsed["fields"]["amount_sats"], 10_000);
        assert!(parsed.get("secret_share").is_none());
        assert!(parsed.get("nonce").is_none());
        assert!(parsed.get("signature_share").is_none());
        assert!(parsed.get("raw_tx").is_none());

        let _ = std::fs::remove_file(&path);
    }
}
