use zenon_core::{config::TrustLevel, error::Result};
use serde_json::Value;

use crate::queue::ReviewQueue;

/// Outcome of passing a change set through the approval gate.
#[derive(Debug, Clone)]
pub struct GateResult {
    pub action: GateAction,
    /// Set when action is `Queued`.
    pub review_id: Option<String>,
}

/// What the gate decided to do with the proposed changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateAction {
    /// Changes should be applied immediately (Auto trust).
    Apply,
    /// Changes have been queued for human review (Review / Manual trust).
    Queued,
}

/// Evaluate whether `changes` should be applied immediately or queued for review.
///
/// - `TrustLevel::Auto`   → `GateAction::Apply`  (no queue entry created)
/// - `TrustLevel::Review` → enqueues and returns `GateAction::Queued`
/// - `TrustLevel::Manual` → enqueues and returns `GateAction::Queued`
pub fn gate(
    trust: TrustLevel,
    changes: Value,
    queue: &ReviewQueue,
    pipeline: &str,
    note_id: &str,
) -> Result<GateResult> {
    match trust {
        TrustLevel::Auto => {
            tracing::debug!("gate: auto-applying changes for pipeline={pipeline} note={note_id}");
            Ok(GateResult {
                action: GateAction::Apply,
                review_id: None,
            })
        }
        TrustLevel::Review | TrustLevel::Manual => {
            let review_id = queue.enqueue(pipeline, note_id, changes)?;
            tracing::info!(
                "gate: queued review {review_id} for pipeline={pipeline} note={note_id}"
            );
            Ok(GateResult {
                action: GateAction::Queued,
                review_id: Some(review_id),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;

    fn temp_queue() -> (ReviewQueue, NamedTempFile) {
        let f = NamedTempFile::new().expect("temp file");
        let q = ReviewQueue::open(f.path()).expect("open review queue");
        (q, f)
    }

    #[test]
    fn auto_trust_applies_without_queuing() {
        let (q, _f) = temp_queue();
        let result = gate(
            TrustLevel::Auto,
            json!({"para": "projects"}),
            &q,
            "classify",
            "note-1",
        )
        .expect("auto trust gate");

        assert_eq!(result.action, GateAction::Apply);
        assert!(result.review_id.is_none());
        // Nothing should be in the queue
        assert!(q.list(None).expect("list reviews").is_empty());
    }

    #[test]
    fn review_trust_queues_and_returns_id() {
        let (q, _f) = temp_queue();
        let result = gate(
            TrustLevel::Review,
            json!({"summary": "hello"}),
            &q,
            "distill",
            "note-2",
        )
        .expect("review trust gate");

        assert_eq!(result.action, GateAction::Queued);
        let rid = result.review_id.expect("review id");
        let item = q.get(&rid).expect("get review");
        assert_eq!(item.status, "pending");
        assert_eq!(item.pipeline, "distill");
    }

    #[test]
    fn manual_trust_also_queues() {
        let (q, _f) = temp_queue();
        let result = gate(
            TrustLevel::Manual,
            json!({"key": "val"}),
            &q,
            "link",
            "note-3",
        )
        .expect("manual trust gate");

        assert_eq!(result.action, GateAction::Queued);
        assert!(result.review_id.is_some());
    }
}
