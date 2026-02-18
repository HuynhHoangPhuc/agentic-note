use agentic_note_core::Result;
use agentic_note_sync::encryption::{
    decrypt_envelope, encrypt_envelope, EnvelopeVersion,
};
use agentic_note_sync::double_ratchet::{
    derive_x3dh_root, generate_prekey, init_x3dh_initiator, init_x3dh_responder,
};

#[test]
fn legacy_and_dr_round_trip() -> Result<()> {
    let key = [7u8; 32];

    let legacy = encrypt_envelope(EnvelopeVersion::Legacy, &key, None, b"hello", b"ad")?;
    let legacy_plain = decrypt_envelope(&key, None, &legacy, b"ad")?;
    assert_eq!(legacy_plain, b"hello");

    let (bob_keypair, bob_prekey) = generate_prekey()?;
    let root = derive_x3dh_root(bob_prekey);
    let mut alice = init_x3dh_initiator(root, bob_prekey)?;
    let mut bob = init_x3dh_responder(root, bob_keypair)?;

    let dr = encrypt_envelope(
        EnvelopeVersion::DoubleRatchet,
        &key,
        Some(&mut alice),
        b"hello",
        b"ad",
    )?;
    let dr_plain = decrypt_envelope(&key, Some(&mut bob), &dr, b"ad")?;
    assert_eq!(dr_plain, b"hello");

    Ok(())
}
