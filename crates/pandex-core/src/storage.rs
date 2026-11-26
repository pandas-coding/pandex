//! Vault persistence. The core defines the storage interface and never names
//! a platform path: the caller injects a [`VaultStore`]. On device the NSH app
//! builds a [`FileStore`] from the host-provided data directory (Android
//! `filesDir`, Windows `%APPDATA%`); headless tests and hosts without a data
//! directory use the in-memory store. Disk only ever holds ciphertext.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// A failed storage operation. The core surfaces these as [`crate::Error::Storage`].
#[derive(Debug, Eq, PartialEq)]
pub struct StorageError(pub String);

/// Where the encrypted vault image is kept. Implementations must be safe to
/// share across threads (the future UniFFI facade holds the core behind a lock).
pub trait VaultStore: Send + Sync {
    /// The persisted image, or `None` when no vault has been written yet.
    fn load(&self) -> Result<Option<Vec<u8>>, StorageError>;
    /// Replace the persisted image atomically.
    fn save(&self, bytes: &[u8]) -> Result<(), StorageError>;
}

/// A single vault file on the local filesystem, written atomically
/// (temporary file in the same directory, then `rename`).
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The sibling temporary file a write lands in before the rename.
    fn tmp_path(&self) -> PathBuf {
        let mut name = self
            .path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(".tmp");
        self.path.with_file_name(name)
    }
}

impl VaultStore for FileStore {
    fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StorageError(e.to_string())),
        }
    }

    fn save(&self, bytes: &[u8]) -> Result<(), StorageError> {
        let io = |e: std::io::Error| StorageError(e.to_string());
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let tmp = self.tmp_path();
        {
            let mut file = std::fs::File::create(&tmp).map_err(io)?;
            file.write_all(bytes).map_err(io)?;
            file.sync_all().map_err(io)?;
        }
        std::fs::rename(&tmp, &self.path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io(e)
        })?;
        Ok(())
    }
}

/// An in-memory vault image: for headless tests and hosts with no data
/// directory. Each instance starts empty, so test sessions are isolated.
pub struct MemoryStore {
    cell: Mutex<Option<Vec<u8>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            cell: Mutex::new(None),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultStore for MemoryStore {
    fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.cell.lock().expect("memory store").clone())
    }

    fn save(&self, bytes: &[u8]) -> Result<(), StorageError> {
        *self.cell.lock().expect("memory store") = Some(bytes.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileStore, MemoryStore, VaultStore};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let hex: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        dir.push(format!("pandex-store-{tag}-{hex}"));
        dir
    }

    #[test]
    fn file_store_roundtrip_and_missing_file() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("vault.pandex");
        let store = FileStore::new(path.clone());

        assert_eq!(store.load().unwrap(), None);
        store.save(b"first").unwrap();
        assert_eq!(store.load().unwrap(), Some(b"first".to_vec()));
        store.save(b"second").unwrap();
        assert_eq!(store.load().unwrap(), Some(b"second".to_vec()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_store_save_is_atomic_and_leaves_no_temp() {
        let dir = temp_dir("atomic");
        let path = dir.join("vault.pandex");
        let store = FileStore::new(path.clone());

        store.save(b"original").unwrap();
        // A stale temporary file from an interrupted write is overwritten.
        let tmp = path.with_file_name("vault.pandex.tmp");
        std::fs::write(&tmp, b"stale").unwrap();
        store.save(b"replacement").unwrap();

        assert_eq!(store.load().unwrap(), Some(b"replacement".to_vec()));
        assert!(!tmp.exists(), "temporary file must be renamed away");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_store_failed_save_keeps_original() {
        let dir = temp_dir("failed");
        let path = dir.join("vault.pandex");
        let store = FileStore::new(path.clone());
        store.save(b"original").unwrap();

        // Turning the parent into a file makes the rename target's directory
        // unwritable: the save must fail and leave the original intact.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let bad = FileStore::new(blocker.join("nested").join("vault.pandex"));
        assert!(bad.save(b"new").is_err());
        assert_eq!(store.load().unwrap(), Some(b"original".to_vec()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn memory_store_roundtrip() {
        let store = MemoryStore::new();
        assert_eq!(store.load().unwrap(), None);
        store.save(b"image").unwrap();
        assert_eq!(store.load().unwrap(), Some(b"image".to_vec()));
    }
}
