//! Encryption at rest for drivers' phones (it protects backups and a leaked
//! `.db` file, not a compromised server: the key lives in `.env`), and keyed
//! fingerprints of client-driver pairs.

use anyhow::{Context, bail};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngExt;

const NONCE_LEN: usize = 24;

#[derive(Clone)]
pub struct PhoneCipher(XChaCha20Poly1305);

impl PhoneCipher {
    /// `hex_key` is 32 bytes in hex (`openssl rand -hex 32`).
    pub fn from_hex(hex_key: &str) -> anyhow::Result<PhoneCipher> {
        let key = hex::decode(hex_key.trim()).context("PHONE_KEY must be hex")?;
        if key.len() != 32 {
            bail!("PHONE_KEY must be 32 bytes (64 hex characters)");
        }
        let cipher = XChaCha20Poly1305::new_from_slice(&key).context("invalid PHONE_KEY")?;
        Ok(PhoneCipher(cipher))
    }

    /// Returns `nonce || ciphertext`.
    pub fn encrypt(&self, phone: &str) -> Vec<u8> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::rng().fill(&mut nonce);
        let mut out = nonce.to_vec();
        let sealed = self
            .0
            .encrypt(&XNonce::from(nonce), phone.as_bytes())
            .expect("encrypting a short phone cannot fail");
        out.extend(sealed);
        out
    }

    pub fn decrypt(&self, blob: &[u8]) -> anyhow::Result<String> {
        if blob.len() < NONCE_LEN {
            bail!("encrypted phone is too short");
        }
        let (nonce, sealed) = blob.split_at(NONCE_LEN);
        let nonce = XNonce::try_from(nonce).context("bad nonce")?;
        let plain = self.0.decrypt(&nonce, sealed).ok().context("cannot decrypt phone")?;
        String::from_utf8(plain).context("decrypted phone is not text")
    }
}

/// Fingerprints a client-driver pair for "one rating per pair per week".
/// An HMAC, not a plain hash: Telegram ids are small enough to brute-force.
#[derive(Clone)]
pub struct PairKey(Vec<u8>);

impl PairKey {
    /// `hex_key` is 32 bytes in hex (`openssl rand -hex 32`).
    pub fn from_hex(hex_key: &str) -> anyhow::Result<PairKey> {
        let key = hex::decode(hex_key.trim()).context("PAIR_KEY must be hex")?;
        if key.len() != 32 {
            bail!("PAIR_KEY must be 32 bytes (64 hex characters)");
        }
        Ok(PairKey(key))
    }

    pub fn fingerprint(&self, client: i64, driver: i64) -> String {
        use hmac::{KeyInit, Mac};
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&self.0).expect("HMAC takes any key length");
        mac.update(format!("{client}:{driver}").as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_fingerprints_depend_on_the_key_and_the_order() {
        let a = PairKey::from_hex(KEY).unwrap();
        let b = PairKey::from_hex(&"ff".repeat(32)).unwrap();
        assert_eq!(a.fingerprint(1, 2), a.fingerprint(1, 2));
        assert_ne!(a.fingerprint(1, 2), a.fingerprint(2, 1));
        assert_ne!(a.fingerprint(1, 2), b.fingerprint(1, 2));
        assert_eq!(a.fingerprint(1, 2).len(), 64);
    }

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[test]
    fn round_trip_with_random_nonces() {
        let cipher = PhoneCipher::from_hex(KEY).unwrap();
        let (a, b) = (cipher.encrypt("+5351234567"), cipher.encrypt("+5351234567"));
        assert_ne!(a, b);
        assert_eq!(cipher.decrypt(&a).unwrap(), "+5351234567");
    }

    #[test]
    fn rejects_bad_keys_and_tampered_data() {
        assert!(PhoneCipher::from_hex("abcd").is_err());
        let cipher = PhoneCipher::from_hex(KEY).unwrap();
        let mut blob = cipher.encrypt("+5351234567");
        *blob.last_mut().unwrap() ^= 1;
        assert!(cipher.decrypt(&blob).is_err());
    }
}
