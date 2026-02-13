use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_debounce() -> u64 {
    500
}

/// Describes what file system event activates a pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub trigger_type: TriggerType,
    /// Optional glob-style path filter, e.g. `"projects/**"`.
    pub path_filter: Option<String>,
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
}

/// The event kinds that can trigger a pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    FileCreated,
    FileModified,
    Manual,
}

/// A file system event produced by a watcher (or test harness).
pub struct FileEvent {
    pub path: PathBuf,
    pub event_type: FileEventType,
}

/// The kind of change that occurred to a file.
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
}

impl TriggerConfig {
    /// Returns `true` when `event` satisfies both the trigger type and
    /// optional path filter.
    ///
    /// Path matching: the filter string is treated as a prefix segment
    /// pattern — a `**` suffix means "anything under this directory",
    /// otherwise the filter must be a literal prefix of the event path
    /// string.
    pub fn matches(&self, event: &FileEvent) -> bool {
        // Manual triggers are never fired by file events.
        let type_match = match (&self.trigger_type, &event.event_type) {
            (TriggerType::FileCreated, FileEventType::Created) => true,
            (TriggerType::FileModified, FileEventType::Modified) => true,
            (TriggerType::Manual, _) => false,
            _ => false,
        };

        if !type_match {
            return false;
        }

        match &self.path_filter {
            None => true,
            Some(filter) => path_matches(filter, &event.path),
        }
    }
}

/// Minimal glob-like path filter: supports `**` suffix wildcard only.
fn path_matches(filter: &str, path: &PathBuf) -> bool {
    let path_str = path.to_string_lossy();
    if let Some(prefix) = filter.strip_suffix("/**") {
        // Match anything under the given directory prefix.
        path_str.starts_with(prefix)
    } else if filter.ends_with("**") {
        let prefix = filter.trim_end_matches("**");
        path_str.starts_with(prefix)
    } else {
        // Literal prefix match.
        path_str.starts_with(filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trigger(t: TriggerType, filter: Option<&str>) -> TriggerConfig {
        TriggerConfig {
            trigger_type: t,
            path_filter: filter.map(str::to_string),
            debounce_ms: 500,
        }
    }

    #[test]
    fn trigger_matches_type_and_filter() {
        let cfg = make_trigger(TriggerType::FileCreated, Some("projects/**"));
        let yes = FileEvent {
            path: PathBuf::from("projects/foo/bar.md"),
            event_type: FileEventType::Created,
        };
        let wrong_type = FileEvent {
            path: PathBuf::from("projects/foo/bar.md"),
            event_type: FileEventType::Modified,
        };
        let wrong_path = FileEvent {
            path: PathBuf::from("areas/foo.md"),
            event_type: FileEventType::Created,
        };

        assert!(cfg.matches(&yes));
        assert!(!cfg.matches(&wrong_type));
        assert!(!cfg.matches(&wrong_path));
    }

    #[test]
    fn manual_trigger_never_matches_file_events() {
        let cfg = make_trigger(TriggerType::Manual, None);
        let ev = FileEvent {
            path: PathBuf::from("anything.md"),
            event_type: FileEventType::Created,
        };
        assert!(!cfg.matches(&ev));
    }

    #[test]
    fn no_filter_matches_any_path() {
        let cfg = make_trigger(TriggerType::FileModified, None);
        let ev = FileEvent {
            path: PathBuf::from("inbox/note.md"),
            event_type: FileEventType::Modified,
        };
        assert!(cfg.matches(&ev));
    }
}
