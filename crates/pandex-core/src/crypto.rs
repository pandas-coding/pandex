//! Cryptographic primitives for the vault key hierarchy.
//!
//! Master password --(Argon2id)--> KEK; a random 256-bit VEK (the vault key)
//! is wrapped by the KEK with XChaCha20-Poly1305. Unlocking derives the KEK and
//! unwraps the VEK; an AEAD tag failure means the password is wrong. All secret
//! key material lives in [`Zeroizing`] buffers so it is wiped on drop / lock.
//!
//! P5 keeps vault *content* as in-memory demo entries; encrypting content with
//! the VEK and persisting it lands in P6. The VEK wrap/unwrap below is real.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::{Zeroize, Zeroizing};

use crate::{KdfParams, UnlockError};

/// Length of the Argon2id salt in bytes.
pub const SALT_LEN: usize = 16;
/// Length of the vault encryption key (VEK) in bytes.
pub const VEK_LEN: usize = 32;
/// Length of the XChaCha20-Poly1305 nonce in bytes.
pub const NONCE_LEN: usize = 24;

/// Domain-separation AAD bound into the VEK wrap, so a wrapped VEK cannot be
/// replayed as a different ciphertext context.
const WRAP_AAD: &[u8] = b"pandex-vek-wrap-v1";

/// Fill `buf` with OS randomness.
pub(crate) fn random_bytes(buf: &mut [u8]) -> Result<(), UnlockError> {
    getrandom::fill(buf).map_err(|_| UnlockError::Crypto)
}

/// Derive a 32-byte KEK from the master password using Argon2id.
pub(crate) fn derive_kek(
    password: &str,
    salt: &[u8],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; VEK_LEN]>, UnlockError> {
    let params = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(VEK_LEN))
        .map_err(|_| UnlockError::Crypto)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut kek = Zeroizing::new([0u8; VEK_LEN]);
    argon2
        .hash_password_into(password.as_bytes(), salt, kek.as_mut())
        .map_err(|_| UnlockError::Crypto)?;
    Ok(kek)
}

/// Wrap the VEK under the KEK, returning the ciphertext (with appended tag).
pub(crate) fn wrap_vek(
    kek: &[u8; VEK_LEN],
    vek: &[u8; VEK_LEN],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, UnlockError> {
    let cipher = XChaCha20Poly1305::new_from_slice(kek).map_err(|_| UnlockError::Crypto)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| UnlockError::Crypto)?;
    cipher
        .encrypt(
            nonce,
            Payload {
                msg: vek,
                aad: WRAP_AAD,
            },
        )
        .map_err(|_| UnlockError::Crypto)
}

/// Unwrap the VEK under the KEK. A tag failure (wrong password / tampering)
/// surfaces as [`UnlockError::InvalidPassword`].
pub(crate) fn unwrap_vek(
    kek: &[u8; VEK_LEN],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<[u8; VEK_LEN]>, UnlockError> {
    let cipher = XChaCha20Poly1305::new_from_slice(kek).map_err(|_| UnlockError::Crypto)?;
    let nonce = <&XNonce>::try_from(nonce).map_err(|_| UnlockError::InvalidPassword)?;

    let mut plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: WRAP_AAD,
            },
        )
        .map_err(|_| UnlockError::InvalidPassword)?;

    if plaintext.len() != VEK_LEN {
        plaintext.zeroize();
        return Err(UnlockError::InvalidPassword);
    }

    let mut vek = Zeroizing::new([0u8; VEK_LEN]);
    vek.copy_from_slice(&plaintext);
    plaintext.zeroize();
    Ok(vek)
}
