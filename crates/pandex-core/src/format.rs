//! Versioned vault file format (single file, local-first).
//!
//! Layout (little-endian):
//!
//! ```text
//! magic[6] = "PANDEX"
//! version u16 = 1
//! kdf m_cost u32 | t_cost u32 | p_cost u32
//! salt[16]
//! wrap_nonce[24]
//! wrap_ct_len u32 | wrap_ct[wrap_ct_len]      (VEK wrapped under the KEK)
//! content_nonce[24]
//! content_ct_len u32 | content_ct[content_ct_len]
//! ```
//!
//! Everything before `content_ct` is the *header*. The content ciphertext is
//! XChaCha20-Poly1305 under the VEK over the serde_json entry list, with the
//! header bound in as AAD (domain-separated), so any header or content
//! tampering fails the tag. The master password and KEK never appear here.

use crate::crypto::{NONCE_LEN, SALT_LEN, VEK_LEN};
use crate::{Error, KdfParams, WrappedVek};

const MAGIC: &[u8; 6] = b"PANDEX";
const VERSION: u16 = 1;
/// Poly1305 tag length appended to every ciphertext.
pub const TAG_LEN: usize = 16;
/// Domain separation for the content encryption AAD.
const CONTENT_AAD_DOMAIN: &[u8] = b"pandex-vault-content-v1";

/// A parsed (still encrypted) vault image: the wrapped VEK plus the encrypted
/// content. Parsing does no key derivation and decrypts nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultImage {
    pub wrapped: WrappedVek,
    pub content_nonce: Vec<u8>,
    pub content_ciphertext: Vec<u8>,
}

/// The serialized header for a given wrapped VEK, content nonce and content
/// ciphertext length. Deterministic, so it can be rebuilt byte-for-byte as AAD.
pub fn header_bytes(wrapped: &WrappedVek, content_nonce: &[u8], content_ct_len: usize) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(MAGIC);
    h.extend_from_slice(&VERSION.to_le_bytes());
    h.extend_from_slice(&wrapped.kdf.m_cost.to_le_bytes());
    h.extend_from_slice(&wrapped.kdf.t_cost.to_le_bytes());
    h.extend_from_slice(&wrapped.kdf.p_cost.to_le_bytes());
    h.extend_from_slice(&wrapped.salt);
    h.extend_from_slice(&wrapped.nonce);
    h.extend_from_slice(&(wrapped.ciphertext.len() as u32).to_le_bytes());
    h.extend_from_slice(&wrapped.ciphertext);
    h.extend_from_slice(content_nonce);
    h.extend_from_slice(&(content_ct_len as u32).to_le_bytes());
    h
}

/// The content AAD: domain separator followed by the raw header bytes.
pub fn content_aad(header: &[u8]) -> Vec<u8> {
    let mut aad = CONTENT_AAD_DOMAIN.to_vec();
    aad.extend_from_slice(header);
    aad
}

impl VaultImage {
    /// Parse a persisted image. Structural problems (bad magic, unknown
    /// version, inconsistent lengths) yield [`Error::Corrupt`]; authentication
    /// is left to the AEAD at unlock time.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader { bytes, pos: 0 };
        if r.take(6)? != MAGIC {
            return Err(Error::Corrupt);
        }
        if r.u16()? != VERSION {
            return Err(Error::Corrupt);
        }
        let kdf = KdfParams {
            m_cost: r.u32()?,
            t_cost: r.u32()?,
            p_cost: r.u32()?,
        };
        let salt = r.take(SALT_LEN)?.to_vec();
        let nonce = r.take(NONCE_LEN)?.to_vec();
        let wrap_ct_len = r.u32()? as usize;
        if wrap_ct_len != VEK_LEN + TAG_LEN {
            return Err(Error::Corrupt);
        }
        let ciphertext = r.take(wrap_ct_len)?.to_vec();
        let content_nonce = r.take(NONCE_LEN)?.to_vec();
        let content_ct_len = r.u32()? as usize;
        let content_ciphertext = r.take(content_ct_len)?.to_vec();
        if r.pos != bytes.len() {
            return Err(Error::Corrupt);
        }
        Ok(Self {
            wrapped: WrappedVek {
                kdf,
                salt,
                nonce,
                ciphertext,
            },
            content_nonce,
            content_ciphertext,
        })
    }

    /// The raw header bytes (everything before the content ciphertext).
    pub fn header_bytes(&self) -> Vec<u8> {
        header_bytes(
            &self.wrapped,
            &self.content_nonce,
            self.content_ciphertext.len(),
        )
    }

    /// The AAD binding the content ciphertext to this exact header.
    pub fn content_aad(&self) -> Vec<u8> {
        content_aad(&self.header_bytes())
    }

    /// Serialize back to the persisted file bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.header_bytes();
        bytes.extend_from_slice(&self.content_ciphertext);
        bytes
    }
}

/// A bounds-checked little-endian cursor over the file bytes.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Corrupt)?;
        let slice = self.bytes.get(self.pos..end).ok_or(Error::Corrupt)?;
        self.pos = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::{TAG_LEN, VaultImage, header_bytes};
    use crate::crypto::{NONCE_LEN, SALT_LEN, VEK_LEN};
    use crate::{Error, KdfParams, WrappedVek};

    fn sample_image() -> VaultImage {
        VaultImage {
            wrapped: WrappedVek {
                kdf: KdfParams::default(),
                salt: vec![0x11; SALT_LEN],
                nonce: vec![0x22; NONCE_LEN],
                ciphertext: vec![0x33; VEK_LEN + TAG_LEN],
            },
            content_nonce: vec![0x44; NONCE_LEN],
            content_ciphertext: vec![0x55; 64],
        }
    }

    #[test]
    fn image_roundtrips_through_bytes() {
        let image = sample_image();
        let parsed = VaultImage::parse(&image.to_bytes()).unwrap();
        assert_eq!(parsed, image);
    }

    #[test]
    fn header_is_deterministic() {
        let image = sample_image();
        assert_eq!(image.header_bytes(), image.header_bytes());
        assert_eq!(
            image.header_bytes(),
            header_bytes(
                &image.wrapped,
                &image.content_nonce,
                image.content_ciphertext.len()
            )
        );
    }

    #[test]
    fn bad_magic_is_corrupt() {
        let mut bytes = sample_image().to_bytes();
        bytes[0] = b'X';
        assert_eq!(VaultImage::parse(&bytes), Err(Error::Corrupt));
    }

    #[test]
    fn unknown_version_is_corrupt() {
        let mut bytes = sample_image().to_bytes();
        bytes[6] = 0xFF;
        assert_eq!(VaultImage::parse(&bytes), Err(Error::Corrupt));
    }

    #[test]
    fn truncation_is_corrupt() {
        let bytes = sample_image().to_bytes();
        for cut in [0, 5, 20, 60, bytes.len() - 1] {
            assert_eq!(VaultImage::parse(&bytes[..cut]), Err(Error::Corrupt));
        }
    }

    #[test]
    fn trailing_bytes_are_corrupt() {
        let mut bytes = sample_image().to_bytes();
        bytes.push(0x00);
        assert_eq!(VaultImage::parse(&bytes), Err(Error::Corrupt));
    }

    #[test]
    fn inconsistent_content_length_is_corrupt() {
        let mut bytes = sample_image().to_bytes();
        let len = bytes.len();
        // content_ct_len field sits right before the content ciphertext.
        let field = len - 64 - 4;
        bytes[field] = 0x01; // claims 1 byte instead of 64
        assert_eq!(VaultImage::parse(&bytes), Err(Error::Corrupt));
    }
}
