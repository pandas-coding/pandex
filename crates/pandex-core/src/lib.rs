//! Pandex core: domain model and all security logic (key derivation, encryption,
//! lock/unlock state machine, sensitive-memory zeroization). The NSH app is a
//! thin shell that renders snapshots and forwards user events as commands.
//!
//! Public API is shaped to be FFI-describable: plain structs, owned data, no
//! lifetime leakage.

#![forbid(unsafe_code)]

mod crypto;

use zeroize::{Zeroize, Zeroizing};

use crypto::{NONCE_LEN, SALT_LEN, VEK_LEN};

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
/// material persisted (in P6); the master password and KEK are never stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrappedVek {
    pub kdf: KdfParams,
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum UnlockError {
    EmptyPassword,
    InvalidPassword,
    /// An internal cryptographic operation failed (e.g. OS randomness).
    Crypto,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub is_locked: bool,
    pub summary: Option<String>,
    pub item_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntrySummary {
    pub title: String,
    pub username: String,
}

struct Vault {
    wrapped_vek: WrappedVek,
}

struct Unlocked {
    vek: Zeroizing<[u8; VEK_LEN]>,
    entries: Vec<EntrySummary>,
}

pub struct Core {
    vault: Option<Vault>,
    unlocked: Option<Unlocked>,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    /// A locked core with no vault yet.
    pub fn new() -> Self {
        Self {
            vault: None,
            unlocked: None,
        }
    }

    /// Create a new vault: generate a random VEK and wrap it under a KEK
    /// derived from `password`. Leaves the core locked; call [`Core::unlock`].
    ///
    /// P5 seeds in-memory demo entries; P6 replaces this with the first-run
    /// flow and real (encrypted, persisted) content.
    pub fn create_vault(&mut self, password: &str) -> Result<(), UnlockError> {
        if password.is_empty() {
            return Err(UnlockError::EmptyPassword);
        }

        let kdf = KdfParams::default();

        let mut salt = [0u8; SALT_LEN];
        crypto::random_bytes(&mut salt)?;
        let mut vek = Zeroizing::new([0u8; VEK_LEN]);
        crypto::random_bytes(vek.as_mut())?;
        let mut nonce = [0u8; NONCE_LEN];
        crypto::random_bytes(&mut nonce)?;

        let kek = crypto::derive_kek(password, &salt, &kdf)?;
        let ciphertext = crypto::wrap_vek(&kek, &vek, &nonce)?;

        self.vault = Some(Vault {
            wrapped_vek: WrappedVek {
                kdf,
                salt: salt.to_vec(),
                nonce: nonce.to_vec(),
                ciphertext,
            },
        });
        self.unlocked = None;
        Ok(())
    }

    /// Derive the KEK from `password` and unwrap the VEK. An AEAD tag failure
    /// (wrong password or tampering) yields [`UnlockError::InvalidPassword`].
    pub fn unlock(&mut self, password: &str) -> Result<(), UnlockError> {
        if password.is_empty() {
            return Err(UnlockError::EmptyPassword);
        }

        let vault = self.vault.as_ref().ok_or(UnlockError::InvalidPassword)?;
        let wrapped = &vault.wrapped_vek;

        let kek = crypto::derive_kek(password, &wrapped.salt, &wrapped.kdf)?;
        let vek = crypto::unwrap_vek(&kek, &wrapped.nonce, &wrapped.ciphertext)?;

        self.unlocked = Some(Unlocked {
            vek,
            entries: demo_entries(),
        });
        Ok(())
    }

    /// Lock the core, zeroizing the VEK and dropping decrypted content.
    pub fn lock(&mut self) {
        if let Some(mut unlocked) = self.unlocked.take() {
            unlocked.vek.zeroize();
            unlocked.entries.clear();
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        match &self.unlocked {
            None => Snapshot {
                is_locked: true,
                summary: None,
                item_count: 0,
            },
            Some(unlocked) => Snapshot {
                is_locked: false,
                summary: Some("Vault".to_owned()),
                item_count: unlocked.entries.len(),
            },
        }
    }

    pub fn entries(&self) -> Vec<EntrySummary> {
        match &self.unlocked {
            None => Vec::new(),
            Some(unlocked) => unlocked.entries.clone(),
        }
    }

    /// The wrapped VEK header (for persistence in P6 / inspection in tests).
    pub fn wrapped_vek(&self) -> Option<WrappedVek> {
        self.vault.as_ref().map(|v| v.wrapped_vek.clone())
    }

    #[cfg(test)]
    fn vek_bytes(&self) -> Option<[u8; VEK_LEN]> {
        self.unlocked.as_ref().map(|u| *u.vek)
    }
}

/// In-memory demo content for P5. Replaced by real entries in P6.
fn demo_entries() -> Vec<EntrySummary> {
    [
        ("GitHub", "octocat@example.com"),
        ("Proton Mail", "octocat@proton.me"),
        ("Example Bank", "octocat"),
    ]
    .into_iter()
    .map(|(title, username)| EntrySummary {
        title: title.to_owned(),
        username: username.to_owned(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{Core, EntrySummary, KdfParams, Snapshot, UnlockError};
    use crate::crypto;
    use zeroize::{Zeroize, Zeroizing};

    const TEST_PASSWORD: &str = "correct horse";

    fn unlocked_core() -> Core {
        let mut core = Core::new();
        core.create_vault(TEST_PASSWORD).unwrap();
        core.unlock(TEST_PASSWORD).unwrap();
        core
    }

    fn demo_summaries() -> Vec<EntrySummary> {
        vec![
            EntrySummary {
                title: "GitHub".to_owned(),
                username: "octocat@example.com".to_owned(),
            },
            EntrySummary {
                title: "Proton Mail".to_owned(),
                username: "octocat@proton.me".to_owned(),
            },
            EntrySummary {
                title: "Example Bank".to_owned(),
                username: "octocat".to_owned(),
            },
        ]
    }

    #[test]
    fn starts_locked() {
        let core = Core::new();

        assert_eq!(
            core.snapshot(),
            Snapshot {
                is_locked: true,
                summary: None,
                item_count: 0,
            }
        );
        assert!(core.entries().is_empty());
    }

    #[test]
    fn wrong_non_empty_password_keeps_core_locked() {
        let mut core = Core::new();
        core.create_vault(TEST_PASSWORD).unwrap();

        assert_eq!(
            core.unlock("wrong-password"),
            Err(UnlockError::InvalidPassword)
        );
        assert!(core.snapshot().is_locked);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn correct_password_unlocks_and_exposes_demo_summary() {
        let core = unlocked_core();

        assert_eq!(core.snapshot().summary, Some("Vault".to_owned()));
        assert_eq!(core.snapshot().item_count, 3);
        assert_eq!(core.entries(), demo_summaries());
    }

    #[test]
    fn lock_returns_core_to_locked_state() {
        let mut core = unlocked_core();

        core.lock();

        assert!(core.snapshot().is_locked);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn locked_snapshot_and_entries_do_not_expose_sensitive_data() {
        let mut core = unlocked_core();
        core.lock();

        let snapshot = core.snapshot();
        assert_eq!(snapshot.summary, None);
        assert_eq!(snapshot.item_count, 0);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn empty_password_is_rejected_and_keeps_core_locked() {
        let mut core = Core::new();
        core.create_vault(TEST_PASSWORD).unwrap();

        assert_eq!(core.unlock(""), Err(UnlockError::EmptyPassword));
        assert!(core.snapshot().is_locked);
        assert_eq!(core.create_vault(""), Err(UnlockError::EmptyPassword));
    }

    #[test]
    fn unlock_without_vault_is_rejected() {
        let mut core = Core::new();
        assert_eq!(
            core.unlock(TEST_PASSWORD),
            Err(UnlockError::InvalidPassword)
        );
    }

    #[test]
    fn lock_zeroizes_and_drops_vek() {
        let mut core = unlocked_core();
        assert!(core.vek_bytes().is_some());

        core.lock();

        assert!(core.vek_bytes().is_none());
    }

    #[test]
    fn relock_and_unlock_again_works() {
        let mut core = unlocked_core();
        core.lock();
        assert!(core.snapshot().is_locked);
        core.unlock(TEST_PASSWORD).unwrap();
        assert_eq!(core.entries(), demo_summaries());
    }

    #[test]
    fn zeroizing_buffer_is_wiped_on_zeroize() {
        let mut key = Zeroizing::new([7u8; 32]);
        key.zeroize();
        assert_eq!(*key, [0u8; 32]);
    }

    #[test]
    fn each_vault_gets_a_unique_salt_and_vek() {
        let mut a = Core::new();
        let mut b = Core::new();
        a.create_vault(TEST_PASSWORD).unwrap();
        b.create_vault(TEST_PASSWORD).unwrap();

        let wa = a.wrapped_vek().unwrap();
        let wb = b.wrapped_vek().unwrap();
        assert_ne!(wa.salt, wb.salt);
        assert_ne!(wa.ciphertext, wb.ciphertext);
        assert_eq!(wa.kdf, KdfParams::default());
    }

    #[test]
    fn tampered_wrap_ciphertext_is_rejected() {
        let salt = [0x11u8; 16];
        let nonce = [0x22u8; 24];
        let params = KdfParams::default();
        let kek = crypto::derive_kek(TEST_PASSWORD, &salt, &params).unwrap();
        let vek = Zeroizing::new([0x33u8; 32]);
        let mut ciphertext = crypto::wrap_vek(&kek, &vek, &nonce).unwrap();

        // Flip one bit of the ciphertext: the AEAD tag must fail.
        ciphertext[0] ^= 0x01;
        assert_eq!(
            crypto::unwrap_vek(&kek, &nonce, &ciphertext),
            Err(UnlockError::InvalidPassword)
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
            Err(UnlockError::InvalidPassword)
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
}
