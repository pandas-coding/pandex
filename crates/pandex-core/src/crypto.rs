//! Cryptographic primitives for the vault key hierarchy.
//!
//! Master password --(Argon2id)--> KEK; a random 256-bit VEK (the vault key)
//! is wrapped by the KEK with XChaCha20-Poly1305. Unlocking derives the KEK and
//! unwraps the VEK; an AEAD tag failure means the password is wrong. All secret
//! key material lives in [`Zeroizing`] buffers so it is wiped on drop / lock.
//!
//! The VEK wrap/unwrap and the vault-content encrypt/decrypt below are real;
//! the persisted file format lives in [`crate::format`].

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::{Zeroize, Zeroizing};

use crate::{Error, KdfParams};

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
pub(crate) fn random_bytes(buf: &mut [u8]) -> Result<(), Error> {
    getrandom::fill(buf).map_err(|_| Error::Crypto)
}

/// Derive a 32-byte KEK from the master password using Argon2id.
pub(crate) fn derive_kek(
    password: &str,
    salt: &[u8],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; VEK_LEN]>, Error> {
    let params = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(VEK_LEN))
        .map_err(|_| Error::Crypto)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut kek = Zeroizing::new([0u8; VEK_LEN]);
    argon2
        .hash_password_into(password.as_bytes(), salt, kek.as_mut())
        .map_err(|_| Error::Crypto)?;
    Ok(kek)
}

/// Wrap the VEK under the KEK, returning the ciphertext (with appended tag).
pub(crate) fn wrap_vek(
    kek: &[u8; VEK_LEN],
    vek: &[u8; VEK_LEN],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, Error> {
    let cipher = XChaCha20Poly1305::new_from_slice(kek).map_err(|_| Error::Crypto)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| Error::Crypto)?;
    cipher
        .encrypt(
            nonce,
            Payload {
                msg: vek,
                aad: WRAP_AAD,
            },
        )
        .map_err(|_| Error::Crypto)
}

/// Unwrap the VEK under the KEK. A tag failure (wrong password / tampering)
/// surfaces as [`Error::InvalidPassword`].
pub(crate) fn unwrap_vek(
    kek: &[u8; VEK_LEN],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<[u8; VEK_LEN]>, Error> {
    let cipher = XChaCha20Poly1305::new_from_slice(kek).map_err(|_| Error::Crypto)?;
    let nonce = <&XNonce>::try_from(nonce).map_err(|_| Error::InvalidPassword)?;

    let mut plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: WRAP_AAD,
            },
        )
        .map_err(|_| Error::InvalidPassword)?;

    if plaintext.len() != VEK_LEN {
        plaintext.zeroize();
        return Err(Error::InvalidPassword);
    }

    let mut vek = Zeroizing::new([0u8; VEK_LEN]);
    vek.copy_from_slice(&plaintext);
    plaintext.zeroize();
    Ok(vek)
}

/// Encrypt the vault content (the serialized entry list) under the VEK,
/// binding `aad` (the domain-separated header) into the tag.
pub(crate) fn encrypt_content(
    vek: &[u8; VEK_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, Error> {
    let cipher = XChaCha20Poly1305::new_from_slice(vek).map_err(|_| Error::Crypto)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| Error::Crypto)?;
    cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| Error::Crypto)
}

/// Decrypt the vault content under the VEK. A tag failure (wrong VEK or any
/// header/content tampering) surfaces as [`Error::InvalidPassword`]. The
/// recovered plaintext is returned in a [`Zeroizing`] buffer.
pub(crate) fn decrypt_content(
    vek: &[u8; VEK_LEN],
    nonce: &[u8],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let cipher = XChaCha20Poly1305::new_from_slice(vek).map_err(|_| Error::Crypto)?;
    let nonce = <&XNonce>::try_from(nonce).map_err(|_| Error::InvalidPassword)?;
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| Error::InvalidPassword)?;
    Ok(Zeroizing::new(plaintext))
}
