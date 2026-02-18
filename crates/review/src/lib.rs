//! Human-in-the-loop review queue with configurable trust levels.
//!
//! Use `ReviewQueue` for persistence and `gate` to decide whether changes
//! are applied automatically or queued for manual review.

pub mod gate;
pub mod queue;

pub use gate::{gate, GateAction, GateResult};
pub use queue::{ReviewItem, ReviewQueue};
