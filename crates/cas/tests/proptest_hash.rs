use agentic_note_cas::hash::hash_bytes;
use proptest::prelude::*;

proptest! {
    #[test]
    fn hash_is_deterministic(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let h1 = hash_bytes(&data);
        let h2 = hash_bytes(&data);
        prop_assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_with_input(a in proptest::collection::vec(any::<u8>(), 0..256),
                              b in proptest::collection::vec(any::<u8>(), 0..256)) {
        prop_assume!(a != b);
        let h1 = hash_bytes(&a);
        let h2 = hash_bytes(&b);
        prop_assert_ne!(h1, h2);
    }
}
