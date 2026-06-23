use anyhow::Result;
use std::path::PathBuf;

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
}

impl Storage for FileStorage {
    fn read(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.base_dir.join(key);
        Ok(std::fs::read(path)?)
    }

    fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.base_dir.join(key);
        Ok(std::fs::write(path, data)?)
    }

    fn exists(&self, key: &str) -> bool {
        self.base_dir.join(key).exists()
    }

    fn delete(&self, key: &str) -> Result<()> {
        let path = self.base_dir.join(key);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
