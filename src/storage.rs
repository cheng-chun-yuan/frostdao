use anyhow::Result;
use std::path::PathBuf;

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::RwLock;

/// Storage abstraction for both file system and browser localStorage
pub trait Storage {
    fn read(&self, key: &str) -> Result<Vec<u8>>;
    fn write(&self, key: &str, data: &[u8]) -> Result<()>;
    #[allow(dead_code)]
    fn exists(&self, key: &str) -> bool;
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
}

/// LocalStorage-based storage for WASM
#[cfg(target_arch = "wasm32")]
pub struct LocalStorageImpl;

#[cfg(target_arch = "wasm32")]
impl Storage for LocalStorageImpl {
    fn read(&self, key: &str) -> Result<Vec<u8>> {
        let window = web_sys::window().ok_or(anyhow::anyhow!("No window"))?;
        let storage = window
            .local_storage()
            .map_err(|_| anyhow::anyhow!("Failed to get localStorage"))?
            .ok_or(anyhow::anyhow!("localStorage not available"))?;

        let value = storage
            .get_item(key)
            .map_err(|_| anyhow::anyhow!("Failed to read from localStorage"))?
            .ok_or(anyhow::anyhow!("Key not found: {}", key))?;

        // Decode from base64
        Ok(base64_decode(&value)?)
    }

    fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        let window = web_sys::window().ok_or(anyhow::anyhow!("No window"))?;
        let storage = window
            .local_storage()
            .map_err(|_| anyhow::anyhow!("Failed to get localStorage"))?
            .ok_or(anyhow::anyhow!("localStorage not available"))?;

        // Encode to base64 for storage
        let encoded = base64_encode(data);
        storage
            .set_item(key, &encoded)
            .map_err(|_| anyhow::anyhow!("Failed to write to localStorage"))?;

        Ok(())
    }

    fn exists(&self, key: &str) -> bool {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(item) = storage.get_item(key) {
                    return item.is_some();
                }
            }
        }
        false
    }
}

// Simple base64 encoding/decoding for WASM
#[cfg(target_arch = "wasm32")]
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(target_arch = "wasm32")]
fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.decode(s)?)
}
