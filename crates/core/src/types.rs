use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use ulid::Ulid;

use crate::error::AgenticError;

/// Unique note identifier wrapping a ULID for monotonic ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteId(pub Ulid);

impl NoteId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NoteId {
    type Err = AgenticError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ulid::from_string(s)
            .map(NoteId)
            .map_err(|e| AgenticError::Parse(format!("invalid ULID: {e}")))
    }
}

/// PARA method categories + Zettelkasten support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParaCategory {
    Projects,
    Areas,
    Resources,
    Archives,
    Inbox,
    Zettelkasten,
}

impl fmt::Display for ParaCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projects => write!(f, "projects"),
            Self::Areas => write!(f, "areas"),
            Self::Resources => write!(f, "resources"),
            Self::Archives => write!(f, "archives"),
            Self::Inbox => write!(f, "inbox"),
            Self::Zettelkasten => write!(f, "zettelkasten"),
        }
    }
}

/// Note maturity status following digital garden metaphor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum NoteStatus {
    #[default]
    Seed,
    Budding,
    Evergreen,
}

/// Conflict resolution policy for merge operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ConflictPolicy {
    NewestWins,
    LongestWins,
    MergeBoth,
    /// Tiered merge: diffy paragraph-level 3-way diff, then LLM-assisted, then manual fallback.
    SemanticMerge,
    #[default]
    Manual,
}

/// Error handling policy for pipeline stage failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ErrorPolicy {
    #[default]
    Skip,
    Retry,
    Abort,
    Fallback,
}

/// YAML frontmatter embedded in each markdown note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontMatter {
    pub id: NoteId,
    pub title: String,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub para: ParaCategory,
    #[serde(default)]
    pub links: Vec<NoteId>,
    #[serde(default)]
    pub status: NoteStatus,
}
