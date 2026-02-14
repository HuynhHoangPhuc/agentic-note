use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::ErrorPolicy;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::condition;
use super::context::StageContext;
use super::error_policy::{self, StageError};
use super::executor::{AgentHandler, PipelineResult};
use super::migration;
use super::pipeline::{PipelineConfig, StageConfig};

/// DAG-aware pipeline executor. Stages within the same dependency layer
/// execute in parallel; layers are ordered by topological sort.
pub struct DagExecutor {
    handlers: HashMap<String, Arc<dyn AgentHandler>>,
}

impl DagExecutor {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register an agent handler (overwrites existing entry with same id).
    pub fn register(&mut self, handler: Arc<dyn AgentHandler>) {
        self.handlers
            .insert(handler.agent_id().to_string(), handler);
    }

    /// Build a directed graph where each node is a stage index and each edge
    /// represents a dependency (`depends_on`). Returns the graph and the
    /// topological order (Vec of NodeIndex).
    pub fn build_dag(stages: &[StageConfig]) -> Result<(DiGraph<usize, ()>, Vec<NodeIndex>)> {
        let mut graph: DiGraph<usize, ()> = DiGraph::new();
        let nodes: Vec<NodeIndex> = (0..stages.len()).map(|i| graph.add_node(i)).collect();

        // Index stage names → graph node.
        let name_to_node: HashMap<&str, NodeIndex> = stages
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.as_str(), nodes[i]))
            .collect();

        for (i, stage) in stages.iter().enumerate() {
            for dep in &stage.depends_on {
                let dep_node = name_to_node.get(dep.as_str()).ok_or_else(|| {
                    AgenticError::Pipeline(format!(
                        "stage '{}' depends on unknown stage '{dep}'",
                        stage.name
                    ))
                })?;
                // Edge: dep → current (dep must run before current).
                graph.add_edge(*dep_node, nodes[i], ());
            }
        }

        let order = toposort(&graph, None).map_err(|cycle| {
            AgenticError::Pipeline(format!(
                "cycle detected involving node index {}",
                cycle.node_id().index()
            ))
        })?;

        Ok((graph, order))
    }

    /// Group stage indices into parallel layers based on topological depth.
    /// Stages in the same layer have no dependency between them and can run
    /// concurrently.
    pub fn compute_layers(graph: &DiGraph<usize, ()>, order: &[NodeIndex]) -> Vec<Vec<usize>> {
        let mut depth: HashMap<NodeIndex, usize> = HashMap::new();
        for &node in order {
            let d = graph
                .neighbors_directed(node, petgraph::Direction::Incoming)
                .map(|p| depth.get(&p).copied().unwrap_or(0) + 1)
                .max()
                .unwrap_or(0);
            depth.insert(node, d);
        }
        let max_depth = depth.values().copied().max().unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![vec![]; max_depth + 1];
        for &node in order {
            let d = depth[&node];
            layers[d].push(*graph.node_weight(node).unwrap());
        }
        layers
    }

    /// Execute all pipeline stages respecting DAG dependencies.
    ///
    /// Stages without explicit `depends_on` are treated as v1 sequential;
    /// `migration::migrate_v1_to_v2` is called on a local clone first.
    ///
    /// When a stage uses `on_error = "abort"`, all remaining layers are skipped
    /// and a partial `PipelineResult` is returned.
    pub async fn run_pipeline(
        &self,
        pipeline: &PipelineConfig,
        ctx: &mut StageContext,
    ) -> Result<PipelineResult> {
        // Work on a local clone so we can migrate without mutating the caller's data.
        let mut cfg = pipeline.clone();
        if cfg.schema_version < 2 {
            migration::migrate_v1_to_v2(&mut cfg);
        }

        let total = cfg.stages.len();
        let mut stages_completed = 0usize;
        let mut skipped: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut errors: Vec<StageError> = Vec::new();
        let mut abort = false;

        // Shared accumulated outputs (readable by condition evaluator per layer).
        let shared_outputs: Arc<Mutex<HashMap<String, Value>>> =
            Arc::new(Mutex::new(ctx.outputs.clone()));

        let (graph, order) = Self::build_dag(&cfg.stages)?;
        let layers = Self::compute_layers(&graph, &order);

        'layers: for layer in &layers {
            // Collect results from parallel stage execution within this layer.
            let mut handles = Vec::new();

            for &stage_idx in layer {
                let mut stage = cfg.stages[stage_idx].clone();

                // Resolve effective error policy (stage overrides pipeline default).
                if stage.on_error == ErrorPolicy::default() {
                    stage.on_error = cfg.default_on_error.clone();
                }

                let handler = self.handlers.get(&stage.agent).cloned();
                let outputs_snapshot = shared_outputs.lock().await.clone();
                let ctx_clone = StageContext {
                    note_id: ctx.note_id,
                    note_content: ctx.note_content.clone(),
                    frontmatter: ctx.frontmatter.clone(),
                    outputs: outputs_snapshot.clone(),
                    vault_path: ctx.vault_path.clone(),
                };
                let pipeline_name = cfg.name.clone();
                // Clone handler registry for fallback resolution inside the task.
                let handlers_clone = self.handlers.clone();

                handles.push(tokio::spawn(async move {
                    run_stage(
                        stage,
                        handler,
                        ctx_clone,
                        &outputs_snapshot,
                        &pipeline_name,
                        &handlers_clone,
                    )
                    .await
                }));
            }

            // Collect all results.
            for handle in handles {
                match handle.await {
                    Ok(Ok(StageOutcome::Completed { output_key, value })) => {
                        shared_outputs
                            .lock()
                            .await
                            .insert(output_key.clone(), value.clone());
                        ctx.set_output(&output_key, value);
                        stages_completed += 1;
                    }
                    Ok(Ok(StageOutcome::Skipped { name, warning })) => {
                        skipped.push(name);
                        if let Some(w) = warning {
                            warnings.push(w);
                        }
                    }
                    Ok(Ok(StageOutcome::Aborted {
                        stage_error,
                        warning,
                    })) => {
                        warnings.push(warning);
                        skipped.push(stage_error.stage_name.clone());
                        if errors.len() < 100 {
                            errors.push(stage_error);
                        }
                        abort = true;
                    }
                    Ok(Err(e)) => {
                        warnings.push(format!("stage task error: {e}"));
                    }
                    Err(join_err) => {
                        warnings.push(format!("stage join error: {join_err}"));
                    }
                }
            }

            if abort {
                break 'layers;
            }
        }

        let outputs = shared_outputs.lock().await.clone();
        Ok(PipelineResult {
            stages_completed,
            total,
            outputs,
            skipped,
            warnings,
            errors,
        })
    }
}

impl Default for DagExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of a single stage execution used to communicate back to caller.
enum StageOutcome {
    Completed {
        output_key: String,
        value: Value,
    },
    Skipped {
        name: String,
        warning: Option<String>,
    },
    /// Abort policy triggered; pipeline should stop after this layer.
    Aborted {
        stage_error: StageError,
        warning: String,
    },
}

/// Execute one stage; returns a `StageOutcome`.
async fn run_stage(
    stage: StageConfig,
    handler: Option<Arc<dyn AgentHandler>>,
    mut ctx: StageContext,
    outputs: &HashMap<String, Value>,
    pipeline_name: &str,
    handlers: &HashMap<String, Arc<dyn AgentHandler>>,
) -> Result<StageOutcome> {
    // Evaluate optional condition.
    if let Some(cond) = &stage.condition {
        match condition::evaluate_condition(cond, outputs) {
            Ok(false) => {
                tracing::debug!(
                    "pipeline '{}' stage '{}': condition false, skipping",
                    pipeline_name,
                    stage.name
                );
                return Ok(StageOutcome::Skipped {
                    name: stage.name.clone(),
                    warning: None,
                });
            }
            Err(e) => {
                let msg = format!(
                    "pipeline '{}' stage '{}': condition error: {e}",
                    pipeline_name, stage.name
                );
                tracing::warn!("{msg}");
                return Ok(StageOutcome::Skipped {
                    name: stage.name.clone(),
                    warning: Some(msg),
                });
            }
            Ok(true) => {}
        }
    }

    let Some(h) = handler else {
        let msg = format!(
            "pipeline '{}' stage '{}': no handler for agent '{}'",
            pipeline_name, stage.name, stage.agent
        );
        tracing::warn!("{msg}");
        return Ok(StageOutcome::Skipped {
            name: stage.name.clone(),
            warning: Some(msg),
        });
    };

    match error_policy::execute_with_policy(h.as_ref(), &mut ctx, &stage, handlers).await {
        Ok(Some(value)) => Ok(StageOutcome::Completed {
            output_key: stage.output.clone(),
            value,
        }),
        Ok(None) => Ok(StageOutcome::Skipped {
            name: stage.name.clone(),
            warning: None,
        }),
        Err(stage_error) => {
            let warning = format!(
                "pipeline '{}' stage '{}': abort triggered: {}",
                pipeline_name, stage.name, stage_error.error
            );
            tracing::warn!("{warning}");
            Ok(StageOutcome::Aborted {
                stage_error,
                warning,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::context::StageContext;
    use crate::engine::pipeline::StageConfig;
    use crate::engine::trigger::{TriggerConfig, TriggerType};
    use agentic_note_core::types::{FrontMatter, NoteId, NoteStatus, ParaCategory};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::path::PathBuf;

    // --- helpers ---

    struct EchoAgent;

    #[async_trait]
    impl AgentHandler for EchoAgent {
        fn agent_id(&self) -> &str {
            "echo"
        }
        async fn execute(&self, ctx: &mut StageContext, _cfg: &toml::Value) -> Result<Value> {
            Ok(serde_json::json!({ "echoed": ctx.note_content }))
        }
    }

    fn make_ctx() -> StageContext {
        let fm = FrontMatter {
            id: NoteId::new(),
            title: "T".into(),
            created: Utc::now(),
            modified: Utc::now(),
            tags: vec![],
            para: ParaCategory::Inbox,
            links: vec![],
            status: NoteStatus::Seed,
        };
        StageContext {
            note_id: fm.id,
            note_content: "hello".into(),
            frontmatter: fm,
            outputs: Default::default(),
            vault_path: PathBuf::from("/tmp"),
        }
    }

    fn stage(name: &str, deps: Vec<&str>) -> StageConfig {
        StageConfig {
            name: name.into(),
            agent: "echo".into(),
            config: toml::Value::Table(Default::default()),
            output: format!("{name}_out"),
            depends_on: deps.into_iter().map(String::from).collect(),
            condition: None,
            on_error: Default::default(),
            retry_max: 3,
            retry_backoff_ms: 1000,
            fallback_agent: None,
        }
    }

    fn make_pipeline(stages: Vec<StageConfig>) -> PipelineConfig {
        PipelineConfig {
            name: "test".into(),
            description: "".into(),
            enabled: true,
            schema_version: 2,
            trigger: TriggerConfig {
                trigger_type: TriggerType::Manual,
                path_filter: None,
                debounce_ms: 0,
                cron: None,
                watch_path: None,
            },
            stages,
            default_on_error: Default::default(),
        }
    }

    // --- tests ---

    #[tokio::test]
    async fn parallel_stages_a_b_then_c() {
        let mut exec = DagExecutor::new();
        exec.register(Arc::new(EchoAgent));

        // A and B are independent; C depends on both.
        let pipeline = make_pipeline(vec![
            stage("a", vec![]),
            stage("b", vec![]),
            stage("c", vec!["a", "b"]),
        ]);

        let mut ctx = make_ctx();
        let result = exec.run_pipeline(&pipeline, &mut ctx).await.unwrap();
        assert_eq!(result.stages_completed, 3);
        assert!(result.skipped.is_empty());
        assert!(ctx.get_output("c_out").is_some());
    }

    #[test]
    fn cycle_detection_returns_error() {
        // A→B→A is a cycle.
        let stages = vec![stage("a", vec!["b"]), stage("b", vec!["a"])];
        let err = DagExecutor::build_dag(&stages).unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn v1_migration_produces_sequential_deps() {
        let mut cfg = PipelineConfig {
            name: "v1".into(),
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
            stages: vec![stage("x", vec![]), stage("y", vec![]), stage("z", vec![])],
            default_on_error: Default::default(),
        };
        migration::migrate_v1_to_v2(&mut cfg);
        assert_eq!(cfg.schema_version, 2);
        assert!(cfg.stages[0].depends_on.is_empty());
        assert_eq!(cfg.stages[1].depends_on, vec!["x"]);
        assert_eq!(cfg.stages[2].depends_on, vec!["y"]);
    }

    #[test]
    fn condition_eval_true_false() {
        let mut outputs = HashMap::new();
        outputs.insert("kw".into(), serde_json::json!({ "status": "ok" }));
        assert!(condition::evaluate_condition(r#"kw.status == "ok""#, &outputs).unwrap());
        assert!(!condition::evaluate_condition(r#"kw.status == "bad""#, &outputs).unwrap());
    }

    #[tokio::test]
    async fn stage_with_false_condition_is_skipped() {
        let mut exec = DagExecutor::new();
        exec.register(Arc::new(EchoAgent));

        let mut s = stage("a", vec![]);
        s.condition = Some(r#"nonexistent.field == "never""#.into());

        let pipeline = make_pipeline(vec![s]);
        let mut ctx = make_ctx();
        let result = exec.run_pipeline(&pipeline, &mut ctx).await.unwrap();
        assert_eq!(result.stages_completed, 0);
        assert_eq!(result.skipped, vec!["a"]);
    }
}
