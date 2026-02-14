use super::pipeline::PipelineConfig;

/// Migrate a v1 (sequential) pipeline config to v2 (DAG).
///
/// For each stage at index > 0 the previous stage is added to `depends_on`,
/// creating an explicit linear dependency chain. `schema_version` is set to 2.
pub fn migrate_v1_to_v2(config: &mut PipelineConfig) {
    if config.schema_version >= 2 {
        return;
    }
    // Collect previous stage names first to avoid borrow conflicts.
    let names: Vec<String> = config.stages.iter().map(|s| s.name.clone()).collect();
    for (i, stage) in config.stages.iter_mut().enumerate() {
        if i > 0 && stage.depends_on.is_empty() {
            stage.depends_on = vec![names[i - 1].clone()];
        }
    }
    config.schema_version = 2;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pipeline::StageConfig;
    use crate::engine::trigger::{TriggerConfig, TriggerType};

    fn make_stage(name: &str) -> StageConfig {
        StageConfig {
            name: name.into(),
            agent: "echo".into(),
            config: toml::Value::Table(Default::default()),
            output: format!("{name}_out"),
            depends_on: vec![],
            condition: None,
            on_error: Default::default(),
            retry_max: 3,
            retry_backoff_ms: 1000,
            fallback_agent: None,
        }
    }

    fn make_v1_pipeline() -> PipelineConfig {
        PipelineConfig {
            name: "test".into(),
            description: "".into(),
            enabled: true,
            schema_version: 1,
            trigger: TriggerConfig {
                trigger_type: TriggerType::Manual,
                path_filter: None,
                debounce_ms: 0,
                cron: None,
                watch_path: None,
            },
            stages: vec![make_stage("a"), make_stage("b"), make_stage("c")],
            default_on_error: Default::default(),
        }
    }

    #[test]
    fn v1_migrates_to_sequential_deps() {
        let mut cfg = make_v1_pipeline();
        migrate_v1_to_v2(&mut cfg);

        assert_eq!(cfg.schema_version, 2);
        assert!(cfg.stages[0].depends_on.is_empty());
        assert_eq!(cfg.stages[1].depends_on, vec!["a"]);
        assert_eq!(cfg.stages[2].depends_on, vec!["b"]);
    }

    #[test]
    fn migrate_is_idempotent_for_v2() {
        let mut cfg = make_v1_pipeline();
        cfg.schema_version = 2;
        migrate_v1_to_v2(&mut cfg);
        // No deps should have been added.
        assert!(cfg.stages[1].depends_on.is_empty());
    }
}
