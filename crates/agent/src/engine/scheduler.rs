use std::collections::HashMap;
use std::path::Path;

use zenon_core::error::{AgenticError, Result};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::pipeline::PipelineConfig;
use super::trigger::TriggerType;

/// Info about an active schedule.
#[derive(Debug, Clone)]
pub struct ScheduleInfo {
    pub pipeline_name: String,
    pub trigger_type: TriggerType,
    pub cron_expr: Option<String>,
    pub watch_path: Option<String>,
}

/// Pipeline scheduler that manages cron-based and file-watch-based triggers.
///
/// Reads pipeline TOML configs, activates their triggers, and runs pipelines
/// when triggers fire.
pub struct PipelineScheduler {
    schedules: HashMap<String, ScheduleInfo>,
    cancel: CancellationToken,
}

impl PipelineScheduler {
    /// Create a new scheduler.
    pub fn new() -> Self {
        Self {
            schedules: HashMap::new(),
            cancel: CancellationToken::new(),
        }
    }

    /// Register a pipeline's trigger in the scheduler.
    /// Only Cron and Watch triggers are registered; others are ignored.
    pub fn register_pipeline(&mut self, config: &PipelineConfig) -> Result<()> {
        match config.trigger.trigger_type {
            TriggerType::Cron => {
                let cron_expr = config.trigger.cron.as_ref().ok_or_else(|| {
                    AgenticError::Scheduler(format!(
                        "pipeline '{}': cron trigger requires 'cron' field",
                        config.name
                    ))
                })?;
                // Validate cron expression format (basic check: at least 5 fields)
                if cron_expr.split_whitespace().count() < 5 {
                    return Err(AgenticError::Scheduler(format!(
                        "pipeline '{}': invalid cron expression '{}'",
                        config.name, cron_expr
                    )));
                }
                let info = ScheduleInfo {
                    pipeline_name: config.name.clone(),
                    trigger_type: TriggerType::Cron,
                    cron_expr: Some(cron_expr.clone()),
                    watch_path: None,
                };
                info!(pipeline = %config.name, cron = %cron_expr, "registered cron trigger");
                self.schedules.insert(config.name.clone(), info);
            }
            TriggerType::Watch => {
                let watch_path = config.trigger.watch_path.as_ref().ok_or_else(|| {
                    AgenticError::Scheduler(format!(
                        "pipeline '{}': watch trigger requires 'watch_path' field",
                        config.name
                    ))
                })?;
                let info = ScheduleInfo {
                    pipeline_name: config.name.clone(),
                    trigger_type: TriggerType::Watch,
                    cron_expr: None,
                    watch_path: Some(watch_path.clone()),
                };
                info!(pipeline = %config.name, path = %watch_path, "registered watch trigger");
                self.schedules.insert(config.name.clone(), info);
            }
            _ => {
                // Manual, FileCreated, FileModified — not scheduler-managed
            }
        }
        Ok(())
    }

    /// List all active schedules.
    pub fn list_schedules(&self) -> Vec<&ScheduleInfo> {
        self.schedules.values().collect()
    }

    /// Remove a schedule by pipeline name.
    pub fn remove(&mut self, name: &str) -> bool {
        self.schedules.remove(name).is_some()
    }

    /// Get the cancellation token for graceful shutdown.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Shutdown the scheduler, cancelling all active triggers.
    pub fn shutdown(&self) {
        info!("shutting down pipeline scheduler");
        self.cancel.cancel();
    }

    /// Scan a directory for pipeline TOML files and register all enabled Cron/Watch triggers.
    pub fn scan_and_register(&mut self, pipelines_dir: &Path) -> Result<usize> {
        let configs = PipelineConfig::load_all(pipelines_dir)?;
        let mut count = 0;
        for config in &configs {
            if !config.enabled {
                continue;
            }
            match config.trigger.trigger_type {
                TriggerType::Cron | TriggerType::Watch => {
                    if let Err(e) = self.register_pipeline(config) {
                        warn!(pipeline = %config.name, error = %e, "failed to register trigger");
                    } else {
                        count += 1;
                    }
                }
                _ => {}
            }
        }
        Ok(count)
    }
}

impl Default for PipelineScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trigger::{TriggerConfig, TriggerType};

    fn make_cron_pipeline(name: &str, cron: &str) -> PipelineConfig {
        PipelineConfig {
            name: name.into(),
            description: "test cron pipeline".into(),
            enabled: true,
            trigger: TriggerConfig {
                trigger_type: TriggerType::Cron,
                path_filter: None,
                debounce_ms: 0,
                cron: Some(cron.into()),
                watch_path: None,
            },
            stages: vec![],
            schema_version: 2,
            default_on_error: Default::default(),
        }
    }

    fn make_watch_pipeline(name: &str, path: &str) -> PipelineConfig {
        PipelineConfig {
            name: name.into(),
            description: "test watch pipeline".into(),
            enabled: true,
            trigger: TriggerConfig {
                trigger_type: TriggerType::Watch,
                path_filter: None,
                debounce_ms: 500,
                cron: None,
                watch_path: Some(path.into()),
            },
            stages: vec![],
            schema_version: 2,
            default_on_error: Default::default(),
        }
    }

    #[test]
    fn register_cron_pipeline() {
        let mut scheduler = PipelineScheduler::new();
        let pipeline = make_cron_pipeline("daily-summary", "0 9 * * *");
        scheduler
            .register_pipeline(&pipeline)
            .expect("register pipeline");
        assert_eq!(scheduler.list_schedules().len(), 1);
        assert_eq!(scheduler.list_schedules()[0].pipeline_name, "daily-summary");
    }

    #[test]
    fn register_watch_pipeline() {
        let mut scheduler = PipelineScheduler::new();
        let pipeline = make_watch_pipeline("inbox-watcher", "inbox/");
        scheduler
            .register_pipeline(&pipeline)
            .expect("register pipeline");
        assert_eq!(scheduler.list_schedules().len(), 1);
    }

    #[test]
    fn cron_without_expr_errors() {
        let mut scheduler = PipelineScheduler::new();
        let mut pipeline = make_cron_pipeline("bad", "0 * * * *");
        pipeline.trigger.cron = None;
        assert!(scheduler.register_pipeline(&pipeline).is_err());
    }

    #[test]
    fn invalid_cron_expr_errors() {
        let mut scheduler = PipelineScheduler::new();
        let pipeline = make_cron_pipeline("bad", "invalid");
        assert!(scheduler.register_pipeline(&pipeline).is_err());
    }

    #[test]
    fn remove_schedule() {
        let mut scheduler = PipelineScheduler::new();
        let pipeline = make_cron_pipeline("temp", "*/5 * * * *");
        scheduler
            .register_pipeline(&pipeline)
            .expect("register pipeline");
        assert!(scheduler.remove("temp"));
        assert!(scheduler.list_schedules().is_empty());
    }

    #[test]
    fn manual_trigger_not_registered() {
        let mut scheduler = PipelineScheduler::new();
        let pipeline = PipelineConfig {
            name: "manual-pipe".into(),
            description: "".into(),
            enabled: true,
            trigger: TriggerConfig {
                trigger_type: TriggerType::Manual,
                path_filter: None,
                debounce_ms: 0,
                cron: None,
                watch_path: None,
            },
            stages: vec![],
            schema_version: 2,
            default_on_error: Default::default(),
        };
        scheduler
            .register_pipeline(&pipeline)
            .expect("register pipeline");
        assert!(scheduler.list_schedules().is_empty());
    }
}
