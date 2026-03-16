use zenon_core::error::{AgenticError, Result};
use bincode;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use rand_os::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, SharedSecret, StaticSecret};

use ksi_double_ratchet::{DecryptError, DoubleRatchet, Header, KeyPair};

const NONCE_LEN: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrHeader {
    pub dh: [u8; 32],
    pub n: u32,
    pub pn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrPayload {
    pub header: DrHeader,
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrSessionRole {
    Initiator,
    Responder,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrSessionMaterial {
    pub root_key: [u8; 32],
    pub peer_public: [u8; 32],
    pub role: DrSessionRole,
}

pub struct DrSession {
    pub material: DrSessionMaterial,
    pub ratchet: DoubleRatchet<DrCryptoProvider>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DrPublicKey(PublicKey);

impl AsRef<[u8]> for DrPublicKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<[u8; 32]> for DrPublicKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(PublicKey::from(bytes))
    }
}

impl From<DrPublicKey> for [u8; 32] {
    fn from(key: DrPublicKey) -> Self {
        key.0.to_bytes()
    }
}

pub struct DrKeyPair {
    private: StaticSecret,
    public: DrPublicKey,
}

impl ksi_double_ratchet::KeyPair for DrKeyPair {
    type PublicKey = DrPublicKey;

    fn new<R: rand_core::CryptoRng + rand_core::RngCore>(rng: &mut R) -> Self {
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        let private = StaticSecret::from(seed);
        let public = DrPublicKey(PublicKey::from(&private));
        Self { private, public }
    }

    fn public(&self) -> &DrPublicKey {
        &self.public
    }
}

#[derive(Clone, Debug)]
pub struct DrCryptoProvider;

impl ksi_double_ratchet::CryptoProvider for DrCryptoProvider {
    type PublicKey = DrPublicKey;
    type KeyPair = DrKeyPair;
    type SharedSecret = SharedSecret;

    type RootKey = [u8; 32];
    type ChainKey = [u8; 32];
    type MessageKey = [u8; 32];

    fn diffie_hellman(us: &DrKeyPair, them: &DrPublicKey) -> SharedSecret {
        us.private.diffie_hellman(&them.0)
    }

    fn kdf_rk(
        root_key: &Self::RootKey,
        shared_secret: &Self::SharedSecret,
    ) -> (Self::RootKey, Self::ChainKey) {
        let hk = Hkdf::<Sha256>::new(Some(root_key), shared_secret.as_bytes());
        let mut okm = [0u8; 64];
        if hk.expand(b"zenon-dr-rk", &mut okm).is_err() {
            return ([0u8; 32], [0u8; 32]);
        }
        let mut next_root = [0u8; 32];
        let mut next_chain = [0u8; 32];
        next_root.copy_from_slice(&okm[..32]);
        next_chain.copy_from_slice(&okm[32..]);
        (next_root, next_chain)
    }

    fn kdf_ck(chain_key: &Self::ChainKey) -> (Self::ChainKey, Self::MessageKey) {
        let hk = Hkdf::<Sha256>::new(None, chain_key);
        let mut okm = [0u8; 64];
        if hk.expand(b"zenon-dr-ck", &mut okm).is_err() {
            return ([0u8; 32], [0u8; 32]);
        }
        let mut next_chain = [0u8; 32];
        let mut msg_key = [0u8; 32];
        next_chain.copy_from_slice(&okm[..32]);
        msg_key.copy_from_slice(&okm[32..]);
        (next_chain, msg_key)
    }

    fn encrypt(key: &Self::MessageKey, plaintext: &[u8], associated_data: &[u8]) -> Vec<u8> {
        let nonce = derive_nonce(key);
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        let ciphertext = match cipher.encrypt(
            Nonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad: associated_data,
            },
        ) {
            Ok(ciphertext) => ciphertext,
            Err(_) => return Vec::new(),
        };

        let mut output = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        output
    }

    fn decrypt(
        key: &Self::MessageKey,
        ciphertext: &[u8],
        associated_data: &[u8],
    ) -> std::result::Result<Vec<u8>, DecryptError> {
        if ciphertext.len() < NONCE_LEN {
            return Err(DecryptError::DecryptFailure);
        }
        let nonce = &ciphertext[..NONCE_LEN];
        let body = &ciphertext[NONCE_LEN..];
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                chacha20poly1305::aead::Payload {
                    msg: body,
                    aad: associated_data,
                },
            )
            .map_err(|_| DecryptError::DecryptFailure)
    }
}

pub struct RandCoreAdapter<'a, R: rand::RngCore + ?Sized> {
    rng: &'a mut R,
}

impl<'a, R: rand::RngCore + ?Sized> RandCoreAdapter<'a, R> {
    pub fn new(rng: &'a mut R) -> Self {
        Self { rng }
    }
}

impl<'a, R: rand::RngCore + ?Sized> rand_core::RngCore for RandCoreAdapter<'a, R> {
    fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.rng.fill_bytes(dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> std::result::Result<(), rand_core::Error> {
        self.rng.fill_bytes(dest);
        Ok(())
    }
}

impl<'a, R: rand::RngCore + ?Sized> rand_core::CryptoRng for RandCoreAdapter<'a, R> {}

pub struct OsRngAdapter(OsRng);

impl OsRngAdapter {
    pub fn new() -> Result<Self> {
        OsRng::new()
            .map(Self)
            .map_err(|e| AgenticError::Encryption(format!("os rng init: {e}")))
    }
}

impl rand_core::RngCore for OsRngAdapter {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> std::result::Result<(), rand_core::Error> {
        self.0.try_fill_bytes(dest)
    }
}

impl rand_core::CryptoRng for OsRngAdapter {}

pub fn derive_x3dh_root(peer_prekey: [u8; 32]) -> [u8; 32] {
    let peer_public = PublicKey::from(peer_prekey);
    derive_shared_secret(&peer_public)
}

pub fn generate_prekey() -> Result<(DrKeyPair, [u8; 32])> {
    let mut adapter = OsRngAdapter::new()?;
    let keypair = <DrKeyPair as ksi_double_ratchet::KeyPair>::new(&mut adapter);
    let public: [u8; 32] = keypair.public().clone().into();
    Ok((keypair, public))
}

pub fn init_x3dh_initiator(shared_root: [u8; 32], peer_prekey: [u8; 32]) -> Result<DrSession> {
    let mut adapter = OsRngAdapter::new()?;
    let peer_public = PublicKey::from(peer_prekey);

    let ratchet = DoubleRatchet::<DrCryptoProvider>::new_alice(
        &shared_root,
        DrPublicKey(peer_public),
        None,
        &mut adapter,
    );

    Ok(DrSession {
        material: DrSessionMaterial {
            root_key: shared_root,
            peer_public: peer_prekey,
            role: DrSessionRole::Initiator,
        },
        ratchet,
    })
}

pub fn init_x3dh_responder(shared_root: [u8; 32], keypair: DrKeyPair) -> Result<DrSession> {
    let peer_public: [u8; 32] = keypair.public().clone().into();
    let ratchet = DoubleRatchet::<DrCryptoProvider>::new_bob(shared_root, keypair, None);

    Ok(DrSession {
        material: DrSessionMaterial {
            root_key: shared_root,
            peer_public,
            role: DrSessionRole::Responder,
        },
        ratchet,
    })
}

pub fn dr_encrypt(
    session: &mut DrSession,
    plaintext: &[u8],
    associated_data: &[u8],
) -> Result<DrPayload> {
    let mut adapter = OsRngAdapter::new()?;
    let (header, ciphertext) =
        session
            .ratchet
            .ratchet_encrypt(plaintext, associated_data, &mut adapter);

    Ok(DrPayload {
        header: DrHeader {
            dh: header.dh.as_ref().try_into().unwrap_or([0u8; 32]),
            n: header.n,
            pn: header.pn,
        },
        ciphertext,
    })
}

pub fn export_session(session: &DrSession) -> Result<Vec<u8>> {
    bincode::serialize(&session.material)
        .map_err(|e| AgenticError::Encryption(format!("encode dr material: {e}")))
}

pub fn material_from_session(session: &DrSession) -> DrSessionMaterial {
    session.material.clone()
}

pub fn session_from_material(material: DrSessionMaterial) -> Result<DrSession> {
    let mut adapter = OsRngAdapter::new()?;
    let peer_public = PublicKey::from(material.peer_public);

    let ratchet = match material.role {
        DrSessionRole::Initiator => DoubleRatchet::<DrCryptoProvider>::new_alice(
            &material.root_key,
            DrPublicKey(peer_public),
            None,
            &mut adapter,
        ),
        DrSessionRole::Responder => {
            let keypair = <DrKeyPair as ksi_double_ratchet::KeyPair>::new(&mut adapter);
            DoubleRatchet::<DrCryptoProvider>::new_bob(material.root_key, keypair, None)
        }
    };

    Ok(DrSession { material, ratchet })
}

pub fn import_session(payload: &[u8]) -> Result<DrSession> {
    let material: DrSessionMaterial = bincode::deserialize(payload)
        .map_err(|e| AgenticError::Encryption(format!("decode dr material: {e}")))?;
    session_from_material(material)
}

pub fn dr_decrypt(
    session: &mut DrSession,
    payload: &DrPayload,
    associated_data: &[u8],
) -> Result<Vec<u8>> {
    let header = Header {
        dh: DrPublicKey::from(payload.header.dh),
        n: payload.header.n,
        pn: payload.header.pn,
    };

    session
        .ratchet
        .ratchet_decrypt(&header, &payload.ciphertext, associated_data)
        .map_err(|e| AgenticError::Encryption(format!("double ratchet decrypt: {e:?}")))
}

fn derive_shared_secret(peer_public: &PublicKey) -> [u8; 32] {
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let secret = StaticSecret::from(seed);
    let shared = secret.diffie_hellman(peer_public);
    let hk = Hkdf::<Sha256>::new(None, shared.as_bytes());
    let mut okm = [0u8; 32];
    if hk.expand(b"zenon-x3dh", &mut okm).is_err() {
        return [0u8; 32];
    }
    okm
}

fn derive_nonce(message_key: &[u8; 32]) -> [u8; NONCE_LEN] {
    let hash = Sha256::digest(message_key);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&hash[..NONCE_LEN]);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dr_round_trip() {
        let (bob_keypair, bob_prekey) = generate_prekey().expect("generate prekey");
        let root = derive_x3dh_root(bob_prekey);

        let mut alice = init_x3dh_initiator(root, bob_prekey).expect("init initiator");
        let mut bob = init_x3dh_responder(root, bob_keypair).expect("init responder");

        let payload = dr_encrypt(&mut alice, b"hello", b"ad").expect("dr encrypt");
        let plaintext = dr_decrypt(&mut bob, &payload, b"ad").expect("dr decrypt");
        assert_eq!(plaintext, b"hello");
    }
}
