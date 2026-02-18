//! DAG pipeline engine, LLM providers, built-in agents, plugin system.
//!
//! Provides the `AgentSpace` facade, pipeline configuration, and LLM provider
//! abstractions used to execute agent workflows.

pub mod agents;
pub mod engine;
pub mod llm;
pub mod plugin;

pub use engine::{AgentHandler, AgentSpace, PipelineConfig, PipelineResult, StageContext};
pub use plugin::{PluginAgent, PluginManifest};
