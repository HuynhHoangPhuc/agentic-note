use zenon_cas::{Cas, Snapshot};
use zenon_core::types::ConflictPolicy;
use zenon_sync::merge_driver::merge_after_sync;
use zenon_test_utils::TempVault;

#[test]
fn snapshot_diff_merge_restore_flow() -> zenon_core::Result<()> {
    let vault = TempVault::with_note("inbox/note.md", "Hello")?;
    let cas = Cas::open(vault.path())?;

    let base = Snapshot::create(vault.path(), &cas, Some("base".into()))?;
    vault.write_note("inbox/note.md", "Hello world")?;
    let updated = Snapshot::create(vault.path(), &cas, Some("updated".into()))?;

    let merge = merge_after_sync(
        &cas,
        &base.id,
        &updated.id,
        &updated.id,
        &ConflictPolicy::Manual,
    )?;

    assert_eq!(merge.conflicts, 0);
    Ok(())
}
