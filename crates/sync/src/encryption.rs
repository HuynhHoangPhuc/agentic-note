/// End-to-end encryption for P2P sync using X25519 DH key exchange and ChaCha20-Poly1305 AEAD.
///
/// Known limitation: static X25519 DH provides no forward secrecy. Each device
/// derives a long-lived X25519 secret from its Ed25519 signing key.
use agentic_note_core::error::{AgenticError, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305,
};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use x25519_dalek::{PublicKey, StaticSecret};

/// An encrypted message payload holding nonce + ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// 12-byte ChaCha20-Poly1305 nonce.
    pub nonce: [u8; 12],
    /// Ciphertext produced by ChaCha20-Poly1305 (includes 16-byte auth tag).
    pub ciphertext: Vec<u8>,
}

/// Derive a deterministic X25519 StaticSecret from raw Ed25519 signing key bytes.
///
/// Uses SHA-512 of the signing key, takes the first 32 bytes as seed, then
/// applies X25519 clamping automatically via `StaticSecret::from`.
pub fn derive_x25519_secret(signing_key_bytes: &[u8; 32]) -> StaticSecret {
    let hash = Sha512::digest(signing_key_bytes);
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&hash[..32]);
    StaticSecret::from(seed)
}

/// Derive the shared ChaCha20 key via X25519 Diffie-Hellman + HKDF-SHA256.
///
/// The raw DH output is passed through HKDF to produce a cryptographically
/// proper symmetric key. Both peers must perform this with their respective
/// secret and the other's public key — results will match.
pub fn derive_shared_secret(my_secret: &StaticSecret, peer_public: &PublicKey) -> [u8; 32] {
    let dh_output = my_secret.diffie_hellman(peer_public);
    let hk = Hkdf::<Sha256>::new(None, dh_output.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"agentic-note-sync-v1", &mut okm)
        .expect("32 bytes is valid HKDF-SHA256 output length");
    okm
}

/// Encrypt `plaintext` with ChaCha20-Poly1305 using `shared_key`.
///
/// Generates a random 12-byte nonce. The returned [`EncryptedPayload`] carries
/// the nonce alongside the ciphertext so the receiver can decrypt.
pub fn encrypt_note(shared_key: &[u8; 32], plaintext: &[u8]) -> Result<EncryptedPayload> {
    let cipher = ChaCha20Poly1305::new(shared_key.into());

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = chacha20poly1305::Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| AgenticError::Encryption(format!("encrypt failed: {e}")))?;

    Ok(EncryptedPayload {
        nonce: nonce_bytes,
        ciphertext,
    })
}

/// Decrypt an [`EncryptedPayload`] with ChaCha20-Poly1305 using `shared_key`.
pub fn decrypt_note(shared_key: &[u8; 32], payload: &EncryptedPayload) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(shared_key.into());
    let nonce = chacha20poly1305::Nonce::from(payload.nonce);

    cipher
        .decrypt(&nonce, payload.ciphertext.as_ref())
        .map_err(|e| AgenticError::Encryption(format!("decrypt failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shared_key() -> [u8; 32] {
        // Alice derives her X25519 secret from a fixed signing key seed.
        let alice_seed = [1u8; 32];
        let alice_secret = derive_x25519_secret(&alice_seed);

        // Bob derives his X25519 secret from a different signing key seed.
        let bob_seed = [2u8; 32];
        let bob_secret = derive_x25519_secret(&bob_seed);

        // Compute Alice->Bob shared secret: both should agree.
        let alice_public = PublicKey::from(&alice_secret);
        let bob_public = PublicKey::from(&bob_secret);

        let alice_shared = derive_shared_secret(&alice_secret, &bob_public);
        let bob_shared = derive_shared_secret(&bob_secret, &alice_public);

        assert_eq!(alice_shared, bob_shared, "DH shared secrets must match");
        alice_shared
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = test_shared_key();
        let plaintext = b"Hello, encrypted world!";

        let payload = encrypt_note(&key, plaintext).expect("encrypt must succeed");
        let decrypted = decrypt_note(&key, &payload).expect("decrypt must succeed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_produces_different_nonces() {
        let key = test_shared_key();
        let plaintext = b"same plaintext";

        let p1 = encrypt_note(&key, plaintext).unwrap();
        let p2 = encrypt_note(&key, plaintext).unwrap();

        // Random nonces should (with overwhelming probability) differ.
        assert_ne!(p1.nonce, p2.nonce);
    }

    #[test]
    fn key_derivation_is_deterministic() {
        let seed = [42u8; 32];
        let secret1 = derive_x25519_secret(&seed);
        let secret2 = derive_x25519_secret(&seed);

        let pub1 = PublicKey::from(&secret1);
        let pub2 = PublicKey::from(&secret2);

        assert_eq!(pub1.to_bytes(), pub2.to_bytes(), "derivation must be deterministic");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let key = test_shared_key();
        let wrong_key = [0u8; 32];
        let plaintext = b"secret message";

        let payload = encrypt_note(&key, plaintext).unwrap();
        let result = decrypt_note(&wrong_key, &payload);

        assert!(result.is_err(), "decryption with wrong key should fail");
    }
}
