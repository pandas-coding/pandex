//! Pandex core: domain model and all security logic (key derivation, encryption,
//! lock/unlock state machine, sensitive-memory zeroization, encrypted vault
//! persistence). The NSH app is a thin shell that renders snapshots and forwards
//! user events as commands.
//!
//! Public API is shaped to be FFI-describable: plain structs, owned data, no
//! lifetime leakage. Storage is injected by the caller ([`storage::VaultStore`]);
//! the core names no platform path and depends on no platform SDK.

#![forbid(unsafe_code)]

mod crypto;
mod format;
mod model;
pub mod storage;

pub use model::{Entry, EntryDraft, EntrySummary};
pub use storage::{FileStore, MemoryStore, VaultStore};

use zeroize::{Zeroize, Zeroizing};

use crypto::{NONCE_LEN, SALT_LEN, VEK_LEN};
use format::{TAG_LEN, VaultImage};

/// Argon2id parameters, stored alongside the wrapped VEK so they can be tuned
/// later without breaking existing vaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_cost: u32,
    /// Time cost (iterations).
    pub t_cost: u32,
    /// Degree of parallelism.
    pub p_cost: u32,
}

impl Default for KdfParams {
    /// OWASP low-memory profile: friendly to mid-range Android cold starts.
    fn default() -> Self {
        Self {
            m_cost: 19 * 1024,
            t_cost: 2,
            p_cost: 1,
        }
    }
}

/// The VEK wrapped under the password-derived KEK. This is the only key
/// material persisted; the master password and KEK are never stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrappedVek {
    pub kdf: KdfParams,
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Everything a core command can fail with. Plain enum, FFI-describable.
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    /// A password argument was empty.
    EmptyPassword,
    /// Wrong password, or an AEAD tag failure (tampering).
    InvalidPassword,
    /// An internal cryptographic operation failed (e.g. OS randomness).
    Crypto,
    /// The injected storage failed to read or write.
    Storage,
    /// The persisted image is structurally invalid (magic/version/lengths).
    Corrupt,
    /// [`Core::create_vault`] called when a vault already exists.
    VaultExists,
    /// An operation needs an existing vault but none is loaded.
    NoVault,
    /// An entry operation was attempted while locked.
    Locked,
    /// No entry matches the given id.
    NotFound,
}

/// The render-ready state of the core. `has_vault` drives the first-run
/// (create) versus unlock branch; nothing sensitive is exposed while locked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub has_vault: bool,
    pub is_locked: bool,
    pub summary: Option<String>,
    pub item_count: usize,
}

struct Unlocked {
    vek: Zeroizing<[u8; VEK_LEN]>,
    entries: Vec<Entry>,
}

pub struct Core {
    store: Box<dyn VaultStore>,
    /// The persisted (encrypted) image bytes, if a vault exists. Never holds
    /// decrypted content; the plaintext entries live only in `unlocked`.
    raw: Option<Vec<u8>>,
    unlocked: Option<Unlocked>,
}

impl Core {
    /// A locked core backed by `store`. Loads the persisted image if present
    /// (without decrypting it); a missing image means first run.
    pub fn new(store: Box<dyn VaultStore>) -> Result<Self, Error> {
        let raw = store.load().map_err(|_| Error::Storage)?;
        Ok(Self {
            store,
            raw,
            unlocked: None,
        })
    }

    /// Whether a vault image is loaded (created or reopened).
    pub fn has_vault(&self) -> bool {
        self.raw.is_some()
    }

    /// First run: generate a random VEK, wrap it under a KEK derived from
    /// `password`, and persist an empty encrypted vault. Leaves the core
    /// unlocked (the VEK is already in hand). Fails if a vault exists.
    pub fn create_vault(&mut self, password: &str) -> Result<(), Error> {
        if password.is_empty() {
            return Err(Error::EmptyPassword);
        }
        if self.raw.is_some() {
            return Err(Error::VaultExists);
        }

        let kdf = KdfParams::default();
        let mut salt = [0u8; SALT_LEN];
        crypto::random_bytes(&mut salt)?;
        let mut vek = Zeroizing::new([0u8; VEK_LEN]);
        crypto::random_bytes(vek.as_mut())?;
        let mut wrap_nonce = [0u8; NONCE_LEN];
        crypto::random_bytes(&mut wrap_nonce)?;

        let kek = crypto::derive_kek(password, &salt, &kdf)?;
        let ciphertext = crypto::wrap_vek(&kek, &vek, &wrap_nonce)?;
        let wrapped = WrappedVek {
            kdf,
            salt: salt.to_vec(),
            nonce: wrap_nonce.to_vec(),
            ciphertext,
        };

        let image = seal(&wrapped, &vek, &[])?;
        let bytes = image.to_bytes();
        self.store.save(&bytes).map_err(|_| Error::Storage)?;
        self.raw = Some(bytes);
        self.unlocked = Some(Unlocked {
            vek,
            entries: Vec::new(),
        });
        Ok(())
    }

    /// Derive the KEK from `password`, unwrap the VEK, then decrypt and parse
    /// the content. A tag failure at either layer (wrong password or tampering)
    /// yields [`Error::InvalidPassword`].
    pub fn unlock(&mut self, password: &str) -> Result<(), Error> {
        if password.is_empty() {
            return Err(Error::EmptyPassword);
        }
        let bytes = self.raw.as_ref().ok_or(Error::NoVault)?;
        let image = VaultImage::parse(bytes)?;
        let wrapped = &image.wrapped;

        let kek = crypto::derive_kek(password, &wrapped.salt, &wrapped.kdf)?;
        let vek = crypto::unwrap_vek(&kek, &wrapped.nonce, &wrapped.ciphertext)?;

        let plaintext = crypto::decrypt_content(
            &vek,
            &image.content_nonce,
            &image.content_ciphertext,
            &image.content_aad(),
        )?;
        let entries: Vec<Entry> = serde_json::from_slice(&plaintext).map_err(|_| Error::Corrupt)?;

        self.unlocked = Some(Unlocked { vek, entries });
        Ok(())
    }

    /// Lock the core, zeroizing the VEK and every decrypted entry.
    pub fn lock(&mut self) {
        if let Some(mut unlocked) = self.unlocked.take() {
            unlocked.vek.zeroize();
            for entry in &mut unlocked.entries {
                entry.zeroize();
            }
            unlocked.entries.clear();
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        match &self.unlocked {
            None => Snapshot {
                has_vault: self.raw.is_some(),
                is_locked: true,
                summary: None,
                item_count: 0,
            },
            Some(unlocked) => Snapshot {
                has_vault: true,
                is_locked: false,
                summary: Some("Vault".to_owned()),
                item_count: unlocked.entries.len(),
            },
        }
    }

    /// The list projection: titles and usernames only, never passwords.
    pub fn entries(&self) -> Vec<EntrySummary> {
        match &self.unlocked {
            None => Vec::new(),
            Some(unlocked) => unlocked.entries.iter().map(Entry::summary).collect(),
        }
    }

    /// On-demand disclosure of a single full entry (including its password).
    /// Requires the core to be unlocked; the list API never reveals this.
    pub fn entry(&self, id: &str) -> Result<Entry, Error> {
        let unlocked = self.unlocked.as_ref().ok_or(Error::Locked)?;
        unlocked
            .entries
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    /// Add an entry from caller-supplied fields; the core generates the id and
    /// timestamps, then re-encrypts and atomically persists the vault. Returns
    /// the new id.
    pub fn add_entry(&mut self, draft: EntryDraft) -> Result<String, Error> {
        let id = random_id()?;
        let now = unix_secs();
        {
            let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
            unlocked.entries.push(Entry {
                id: id.clone(),
                title: draft.title,
                username: draft.username,
                password: draft.password,
                urls: draft.urls,
                notes: draft.notes,
                created_at: now,
                updated_at: now,
            });
        }
        self.persist()?;
        Ok(id)
    }

    /// Replace an existing entry's editable fields (id and `created_at` are
    /// preserved, `updated_at` refreshed), then re-encrypt and persist.
    pub fn update_entry(&mut self, entry: Entry) -> Result<(), Error> {
        {
            let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
            let existing = unlocked
                .entries
                .iter_mut()
                .find(|e| e.id == entry.id)
                .ok_or(Error::NotFound)?;
            existing.password.zeroize();
            existing.notes.zeroize();
            existing.title = entry.title;
            existing.username = entry.username;
            existing.password = entry.password;
            existing.urls = entry.urls;
            existing.notes = entry.notes;
            existing.updated_at = unix_secs();
        }
        self.persist()
    }

    /// Remove an entry by id (zeroizing it), then re-encrypt and persist.
    pub fn delete_entry(&mut self, id: &str) -> Result<(), Error> {
        {
            let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
            let pos = unlocked
                .entries
                .iter()
                .position(|e| e.id == id)
                .ok_or(Error::NotFound)?;
            let mut removed = unlocked.entries.remove(pos);
            removed.zeroize();
        }
        self.persist()
    }

    /// The wrapped VEK header (for inspection in tests).
    pub fn wrapped_vek(&self) -> Option<WrappedVek> {
        self.raw
            .as_ref()
            .and_then(|b| VaultImage::parse(b).ok())
            .map(|image| image.wrapped)
    }

    /// Re-encrypt the current entries under the stored wrapped VEK and write
    /// the image atomically. The wrapped VEK (and thus the KEK/password binding)
    /// is unchanged; only the content ciphertext and its nonce are refreshed.
    fn persist(&mut self) -> Result<(), Error> {
        let unlocked = self.unlocked.as_ref().ok_or(Error::Locked)?;
        let bytes = self.raw.as_ref().ok_or(Error::NoVault)?;
        let wrapped = VaultImage::parse(bytes)?.wrapped;
        let image = seal(&wrapped, &unlocked.vek, &unlocked.entries)?;
        let new_bytes = image.to_bytes();
        self.store.save(&new_bytes).map_err(|_| Error::Storage)?;
        self.raw = Some(new_bytes);
        Ok(())
    }

    #[cfg(test)]
    fn vek_bytes(&self) -> Option<[u8; VEK_LEN]> {
        self.unlocked.as_ref().map(|u| *u.vek)
    }
}

/// Encrypt `entries` under the VEK into a fresh vault image (new content nonce).
fn seal(wrapped: &WrappedVek, vek: &[u8; VEK_LEN], entries: &[Entry]) -> Result<VaultImage, Error> {
    let json = Zeroizing::new(serde_json::to_vec(entries).map_err(|_| Error::Crypto)?);
    let mut content_nonce = [0u8; NONCE_LEN];
    crypto::random_bytes(&mut content_nonce)?;
    let header = format::header_bytes(wrapped, &content_nonce, json.len() + TAG_LEN);
    let ciphertext =
        crypto::encrypt_content(vek, &content_nonce, &json, &format::content_aad(&header))?;
    Ok(VaultImage {
        wrapped: wrapped.clone(),
        content_nonce: content_nonce.to_vec(),
        content_ciphertext: ciphertext,
    })
}

/// A random 128-bit hex identifier.
fn random_id() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    crypto::random_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Unix seconds for entry metadata timestamps (not a security decision; the
/// timeout state machine of P7 takes caller-injected time instead).
fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{Core, Entry, EntryDraft, EntrySummary, Error, KdfParams, Snapshot};
    use crate::crypto;
    use crate::storage::{FileStore, MemoryStore};
    use zeroize::{Zeroize, Zeroizing};

    const TEST_PASSWORD: &str = "correct horse";

    fn memory_core() -> Core {
        Core::new(Box::new(MemoryStore::new())).unwrap()
    }

    fn unlocked_core() -> Core {
        let mut core = memory_core();
        core.create_vault(TEST_PASSWORD).unwrap();
        core
    }

    fn draft(title: &str, username: &str, password: &str) -> EntryDraft {
        EntryDraft {
            title: title.to_owned(),
            username: username.to_owned(),
            password: password.to_owned(),
            urls: vec![format!("https://{title}.example.com")],
            notes: String::new(),
        }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let hex: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        dir.push(format!("pandex-core-{tag}-{hex}"));
        dir
    }

    #[test]
    fn starts_locked_without_vault() {
        let core = memory_core();
        assert!(!core.has_vault());
        assert_eq!(
            core.snapshot(),
            Snapshot {
                has_vault: false,
                is_locked: true,
                summary: None,
                item_count: 0,
            }
        );
        assert!(core.entries().is_empty());
    }

    #[test]
    fn create_vault_unlocks_and_persists() {
        let mut core = memory_core();
        core.create_vault(TEST_PASSWORD).unwrap();
        assert!(core.has_vault());
        assert!(!core.snapshot().is_locked);
        assert_eq!(core.snapshot().item_count, 0);
        assert!(core.wrapped_vek().is_some());
    }

    #[test]
    fn create_vault_twice_is_rejected() {
        let mut core = unlocked_core();
        assert_eq!(core.create_vault(TEST_PASSWORD), Err(Error::VaultExists));
    }

    #[test]
    fn empty_password_is_rejected() {
        let mut core = memory_core();
        assert_eq!(core.create_vault(""), Err(Error::EmptyPassword));
        assert_eq!(core.unlock(""), Err(Error::EmptyPassword));
    }

    #[test]
    fn unlock_without_vault_is_rejected() {
        let mut core = memory_core();
        assert_eq!(core.unlock(TEST_PASSWORD), Err(Error::NoVault));
    }

    #[test]
    fn wrong_password_keeps_core_locked() {
        let mut core = memory_core();
        core.create_vault(TEST_PASSWORD).unwrap();
        core.lock();
        assert_eq!(core.unlock("wrong-password"), Err(Error::InvalidPassword));
        assert!(core.snapshot().is_locked);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn lock_returns_to_locked_and_zeroizes() {
        let mut core = unlocked_core();
        let id = core
            .add_entry(draft("GitHub", "octocat", "s3cret"))
            .unwrap();
        assert!(core.vek_bytes().is_some());

        core.lock();

        assert!(core.snapshot().is_locked);
        assert!(core.entries().is_empty());
        assert!(core.vek_bytes().is_none());
        assert_eq!(core.entry(&id), Err(Error::Locked));
    }

    #[test]
    fn relock_and_unlock_again_works() {
        let mut core = unlocked_core();
        core.add_entry(draft("GitHub", "octocat", "s3cret"))
            .unwrap();
        core.lock();
        core.unlock(TEST_PASSWORD).unwrap();
        assert_eq!(core.snapshot().item_count, 1);
        assert_eq!(core.entries()[0].title, "GitHub");
    }

    #[test]
    fn entry_crud_and_on_demand_disclosure() {
        let mut core = unlocked_core();
        let id = core
            .add_entry(draft("GitHub", "octocat@example.com", "s3cret"))
            .unwrap();

        // The list never carries the password.
        let summaries = core.entries();
        assert_eq!(
            summaries,
            vec![EntrySummary {
                id: id.clone(),
                title: "GitHub".to_owned(),
                username: "octocat@example.com".to_owned(),
            }]
        );

        // Full disclosure is a separate, unlocked-only command.
        let full = core.entry(&id).unwrap();
        assert_eq!(full.password, "s3cret");
        assert!(full.created_at > 0);
        assert_eq!(full.created_at, full.updated_at);

        // Update preserves id/created_at and refreshes updated_at.
        let mut edited = full.clone();
        edited.title = "GitHub Enterprise".to_owned();
        edited.password = "n3w-secret".to_owned();
        core.update_entry(edited).unwrap();
        let after = core.entry(&id).unwrap();
        assert_eq!(after.title, "GitHub Enterprise");
        assert_eq!(after.password, "n3w-secret");
        assert_eq!(after.created_at, full.created_at);

        // Delete removes it.
        core.delete_entry(&id).unwrap();
        assert!(core.entries().is_empty());
        assert_eq!(core.entry(&id), Err(Error::NotFound));
        assert_eq!(core.delete_entry(&id), Err(Error::NotFound));
    }

    #[test]
    fn entry_commands_require_unlock() {
        let mut core = memory_core();
        core.create_vault(TEST_PASSWORD).unwrap();
        core.lock();
        assert_eq!(core.add_entry(draft("X", "y", "z")), Err(Error::Locked));
        assert_eq!(core.entry("any"), Err(Error::Locked));
        assert_eq!(core.delete_entry("any"), Err(Error::Locked));
    }

    #[test]
    fn add_entry_generates_unique_ids() {
        let mut core = unlocked_core();
        let a = core.add_entry(draft("A", "a", "pa")).unwrap();
        let b = core.add_entry(draft("B", "b", "pb")).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn file_vault_roundtrip_across_reopen() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("vault.pandex");

        let mut core = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        assert!(!core.has_vault());
        core.create_vault(TEST_PASSWORD).unwrap();
        let id = core
            .add_entry(draft("GitHub", "octocat@example.com", "s3cret"))
            .unwrap();
        core.add_entry(draft("Proton Mail", "octocat@proton.me", "mailpw"))
            .unwrap();
        drop(core);

        // Reopen from disk: locked, content intact after unlock.
        let mut reopened = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        assert!(reopened.has_vault());
        assert!(reopened.snapshot().is_locked);
        assert!(reopened.entries().is_empty());
        reopened.unlock(TEST_PASSWORD).unwrap();
        assert_eq!(reopened.snapshot().item_count, 2);
        assert_eq!(reopened.entry(&id).unwrap().password, "s3cret");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mutations_persist_across_reopen() {
        let dir = temp_dir("mutations");
        let path = dir.join("vault.pandex");

        let mut core = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        core.create_vault(TEST_PASSWORD).unwrap();
        let id = core.add_entry(draft("Bank", "octocat", "old")).unwrap();
        let mut e = core.entry(&id).unwrap();
        e.password = "new".to_owned();
        core.update_entry(e).unwrap();
        let removed = core.add_entry(draft("Temp", "t", "t")).unwrap();
        core.delete_entry(&removed).unwrap();
        drop(core);

        let mut reopened = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        reopened.unlock(TEST_PASSWORD).unwrap();
        assert_eq!(reopened.snapshot().item_count, 1);
        assert_eq!(reopened.entry(&id).unwrap().password, "new");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn disk_holds_only_ciphertext() {
        let dir = temp_dir("ciphertext");
        let path = dir.join("vault.pandex");
        let mut core = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        core.create_vault(TEST_PASSWORD).unwrap();
        core.add_entry(draft("GitHub", "octocat@example.com", "super-secret-pw"))
            .unwrap();
        core.lock();

        let bytes = std::fs::read(&path).unwrap();
        let needle = |s: &str| bytes.windows(s.len()).any(|w| w == s.as_bytes());
        assert!(!needle("GitHub"));
        assert!(!needle("octocat@example.com"));
        assert!(!needle("super-secret-pw"));
        assert!(!needle(TEST_PASSWORD));
        assert_eq!(&bytes[..6], b"PANDEX");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tampered_content_byte_fails_unlock() {
        let dir = temp_dir("tamper-content");
        let path = dir.join("vault.pandex");
        let mut core = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        core.create_vault(TEST_PASSWORD).unwrap();
        core.add_entry(draft("GitHub", "octocat", "s3cret"))
            .unwrap();
        drop(core);

        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01; // flip a bit in the content ciphertext/tag
        std::fs::write(&path, &bytes).unwrap();

        let mut reopened = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        assert_eq!(reopened.unlock(TEST_PASSWORD), Err(Error::InvalidPassword));
        assert!(reopened.snapshot().is_locked);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tampered_header_fails_unlock() {
        let dir = temp_dir("tamper-header");
        let path = dir.join("vault.pandex");
        let mut core = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        core.create_vault(TEST_PASSWORD).unwrap();
        core.add_entry(draft("GitHub", "octocat", "s3cret"))
            .unwrap();
        drop(core);

        let mut bytes = std::fs::read(&path).unwrap();
        bytes[20] ^= 0x01; // flip a bit in the salt
        std::fs::write(&path, &bytes).unwrap();

        let mut reopened = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        assert!(reopened.unlock(TEST_PASSWORD).is_err());
        assert!(reopened.snapshot().is_locked);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_file_is_rejected() {
        let dir = temp_dir("corrupt");
        let path = dir.join("vault.pandex");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"not a pandex vault").unwrap();

        let mut core = Core::new(Box::new(FileStore::new(path.clone()))).unwrap();
        assert!(core.has_vault());
        assert_eq!(core.unlock(TEST_PASSWORD), Err(Error::Corrupt));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_vault_gets_a_unique_salt_and_vek() {
        let mut a = memory_core();
        let mut b = memory_core();
        a.create_vault(TEST_PASSWORD).unwrap();
        b.create_vault(TEST_PASSWORD).unwrap();

        let wa = a.wrapped_vek().unwrap();
        let wb = b.wrapped_vek().unwrap();
        assert_ne!(wa.salt, wb.salt);
        assert_ne!(wa.ciphertext, wb.ciphertext);
        assert_eq!(wa.kdf, KdfParams::default());
    }

    #[test]
    fn zeroizing_buffer_is_wiped_on_zeroize() {
        let mut key = Zeroizing::new([7u8; 32]);
        key.zeroize();
        assert_eq!(*key, [0u8; 32]);
    }

    #[test]
    fn entry_zeroize_wipes_sensitive_fields() {
        let mut entry = Entry {
            id: "id".to_owned(),
            title: "GitHub".to_owned(),
            username: "octocat".to_owned(),
            password: "s3cret".to_owned(),
            urls: vec!["https://github.com".to_owned()],
            notes: "private".to_owned(),
            created_at: 1,
            updated_at: 1,
        };
        entry.zeroize();
        assert!(entry.password.is_empty());
        assert!(entry.notes.is_empty());
        assert!(entry.title.is_empty());
    }

    #[test]
    fn tampered_wrap_ciphertext_is_rejected() {
        let salt = [0x11u8; 16];
        let nonce = [0x22u8; 24];
        let params = KdfParams::default();
        let kek = crypto::derive_kek(TEST_PASSWORD, &salt, &params).unwrap();
        let vek = Zeroizing::new([0x33u8; 32]);
        let mut ciphertext = crypto::wrap_vek(&kek, &vek, &nonce).unwrap();

        ciphertext[0] ^= 0x01;
        assert_eq!(
            crypto::unwrap_vek(&kek, &nonce, &ciphertext),
            Err(Error::InvalidPassword)
        );
    }

    #[test]
    fn wrong_password_kek_cannot_unwrap() {
        let salt = [0x44u8; 16];
        let nonce = [0x55u8; 24];
        let params = KdfParams::default();
        let kek = crypto::derive_kek(TEST_PASSWORD, &salt, &params).unwrap();
        let vek = Zeroizing::new([0x66u8; 32]);
        let ciphertext = crypto::wrap_vek(&kek, &vek, &nonce).unwrap();

        let wrong_kek = crypto::derive_kek("not the password", &salt, &params).unwrap();
        assert_eq!(
            crypto::unwrap_vek(&wrong_kek, &nonce, &ciphertext),
            Err(Error::InvalidPassword)
        );
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    // Known-answer regression vectors pinned from the reference implementations
    // (RustCrypto argon2 0.6.0 / chacha20poly1305 0.11.0). They guard against
    // accidental changes to the KDF/AEAD wiring or parameters.
    #[test]
    fn argon2id_known_answer() {
        let kek = crypto::derive_kek("pandex-kat-password", &[0x01u8; 16], &KdfParams::default())
            .unwrap();
        assert_eq!(
            hex(&*kek),
            "c220eacd155c9aa43708bfb557aa0887b6a5675676d008e33b4d6c3c7c319ea9"
        );
    }

    #[test]
    fn xchacha20poly1305_known_answer() {
        let ct = crypto::wrap_vek(&[0x02u8; 32], &[0x04u8; 32], &[0x03u8; 24]).unwrap();
        assert_eq!(
            hex(&ct),
            "590d255b39573e83843783ca308b7cfd9d75d7368a8af4164aada4076ddc2050\
             e7d95228a0e88a6a89f294c3f84d7290"
        );
    }

    #[test]
    fn wrap_unwrap_roundtrip_recovers_vek() {
        let salt = [0x77u8; 16];
        let nonce = [0x88u8; 24];
        let params = KdfParams::default();
        let kek = crypto::derive_kek(TEST_PASSWORD, &salt, &params).unwrap();
        let vek = Zeroizing::new([0x99u8; 32]);
        let ciphertext = crypto::wrap_vek(&kek, &vek, &nonce).unwrap();

        let recovered = crypto::unwrap_vek(&kek, &nonce, &ciphertext).unwrap();
        assert_eq!(*recovered, *vek);
    }

    #[test]
    fn content_encrypt_decrypt_roundtrip_and_tamper() {
        let vek = [0x05u8; 32];
        let nonce = [0x06u8; 24];
        let aad = b"pandex-vault-content-v1header";
        let plaintext = b"[{\"id\":\"x\"}]";
        let ct = crypto::encrypt_content(&vek, &nonce, plaintext, aad).unwrap();
        let recovered = crypto::decrypt_content(&vek, &nonce, &ct, aad).unwrap();
        assert_eq!(&*recovered, plaintext);

        let mut tampered = ct.clone();
        tampered[0] ^= 0x01;
        assert_eq!(
            crypto::decrypt_content(&vek, &nonce, &tampered, aad),
            Err(Error::InvalidPassword)
        );
        // A different AAD (header) also fails the tag.
        assert_eq!(
            crypto::decrypt_content(&vek, &nonce, &ct, b"different-header"),
            Err(Error::InvalidPassword)
        );
    }
}
