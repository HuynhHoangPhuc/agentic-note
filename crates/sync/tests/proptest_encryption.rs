use agentic_note_sync::encryption::{decrypt_envelope, encrypt_envelope, EnvelopeVersion};
use proptest::prelude::*;

proptest! {
    #[test]
    fn legacy_encrypt_round_trip(data in proptest::collection::vec(any::<u8>(), 0..128)) {
        let key = [7u8; 32];
        let envelope = encrypt_envelope(EnvelopeVersion::Legacy, &key, None, &data, b"ad")
            .expect("encrypt legacy");
        let decrypted = decrypt_envelope(&key, None, &envelope, b"ad")
            .expect("decrypt legacy");
        prop_assert_eq!(decrypted, data);
    }
}
