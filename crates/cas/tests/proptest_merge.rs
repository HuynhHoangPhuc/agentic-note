use zenon_cas::merge::three_way_merge;
use zenon_cas::tree::{EntryType, Tree, TreeEntry};
use zenon_cas::BlobStore;
use zenon_core::types::ConflictPolicy;
use proptest::prelude::*;
use tempfile::TempDir;

fn setup_store() -> (TempDir, BlobStore) {
    let dir = TempDir::new().expect("temp dir");
    let store = BlobStore::new(dir.path().join("objects"));
    (dir, store)
}

fn store_empty_tree(store: &BlobStore) -> String {
    let tree = Tree {
        entries: Vec::new(),
    };
    let bytes = serde_json::to_vec(&tree).expect("encode tree");
    store.store(&bytes).expect("store tree")
}

proptest! {
    #[test]
    fn merge_no_conflict_for_identical_inputs(name in "[a-z]{1,8}", data in proptest::collection::vec(any::<u8>(), 0..128)) {
        let (_dir, store) = setup_store();

        let blob_id = store.store(&data).expect("store blob");
        let tree = Tree {
            entries: vec![TreeEntry {
                name,
                entry_type: EntryType::Blob,
                hash: blob_id,
            }],
        };
        let tree_bytes = serde_json::to_vec(&tree).expect("encode tree");
        let tree_id = store.store(&tree_bytes).expect("store tree");
        let base = store_empty_tree(&store);

        let merge = three_way_merge(&store, &base, &tree_id, &tree_id, &ConflictPolicy::Manual)
            .expect("merge");
        prop_assert!(merge.conflicts.is_empty());
    }
}
