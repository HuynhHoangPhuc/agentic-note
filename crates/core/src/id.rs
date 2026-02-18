use std::sync::Mutex;
use ulid::{Generator, Ulid};

use crate::types::NoteId;

static ID_GEN: Mutex<Option<Generator>> = Mutex::new(None);

/// Generate a monotonically ordered NoteId using ULID.
pub fn next_id() -> NoteId {
    let mut guard = match ID_GEN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("ID generator lock poisoned; recovering");
            poisoned.into_inner()
        }
    };
    let gen = guard.get_or_insert_with(Generator::new);
    let ulid = match gen.generate() {
        Ok(ulid) => ulid,
        Err(err) => {
            tracing::error!(error = %err, "ULID generation failed; falling back to new ULID");
            Ulid::new()
        }
    };
    NoteId(ulid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monotonic_ids() {
        let a = next_id();
        let b = next_id();
        let c = next_id();
        // Monotonic: each subsequent ID is greater
        assert!(b.0 > a.0);
        assert!(c.0 > b.0);
    }
}
