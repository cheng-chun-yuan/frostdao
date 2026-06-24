use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::RwLock;

/// Storage abstraction for file-backed and test storage.
pub trait Storage {
    fn read(&self, key: &str) -> Result<Vec<u8>>;
    fn write(&self, key: &str, data: &[u8]) -> Result<()>;
    #[allow(dead_code)]
    fn exists(&self, key: &str) -> bool;
    /// Delete a key from storage. Used for security-critical cleanup (e.g., nonces).
    fn delete(&self, key: &str) -> Result<()>;
}

/// In-memory storage for testing
#[cfg(test)]
pub struct MemoryStorage {
    data: RwLock<HashMap<String, Vec<u8>>>,
}

#[cfg(test)]
impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Storage for MemoryStorage {
    fn read(&self, key: &str) -> Result<Vec<u8>> {
        let data = self.data.read().unwrap();
        data.get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Key not found: {}", key))
    }

    fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let mut storage = self.data.write().unwrap();
        storage.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    fn exists(&self, key: &str) -> bool {
        let data = self.data.read().unwrap();
        data.contains_key(key)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut storage = self.data.write().unwrap();
        storage.remove(key);
        Ok(())
    }
}

/// File-based storage for CLI
pub struct FileStorage {
    base_dir: PathBuf,
}

impl FileStorage {
    pub fn new(base_dir: &str) -> Result<Self> {
        let path = PathBuf::from(base_dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self { base_dir: path })
    }

    fn key_path(&self, key: &str) -> Result<PathBuf> {
        validate_storage_key(key)?;
        Ok(self.base_dir.join(key))
    }
}

impl Storage for FileStorage {
    fn read(&self, key: &str) -> Result<Vec<u8>> {
        Ok(std::fs::read(self.key_path(key)?)?)
    }

    fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        Ok(std::fs::write(self.key_path(key)?, data)?)
    }

    fn exists(&self, key: &str) -> bool {
        self.key_path(key).is_ok_and(|path| path.exists())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let path = self.key_path(key)?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn validate_storage_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        bail!("storage key must not be empty");
    }

    let path = Path::new(key);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(())
    } else {
        bail!(
            "storage key must be a relative path without traversal: {}",
            key
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{FileStorage, Storage};

    #[test]
    fn file_storage_rejects_traversal_keys() {
        let base = std::env::temp_dir().join(format!(
            "frostdao-storage-traversal-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let storage = FileStorage::new(base.to_str().unwrap()).unwrap();

        for key in [
            "",
            "../wallet.json",
            "/tmp/wallet.json",
            "wallet/../../secret",
        ] {
            assert!(storage.write(key, b"secret").is_err());
            assert!(storage.read(key).is_err());
            assert!(!storage.exists(key));
            assert!(storage.delete(key).is_err());
        }

        storage.write("wallet.json", b"ok").unwrap();
        assert_eq!(storage.read("wallet.json").unwrap(), b"ok");

        let _ = std::fs::remove_dir_all(&base);
    }
}
