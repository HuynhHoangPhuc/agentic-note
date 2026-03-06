use agentic_note_agent::agents::{Distiller, ParaClassifier};
use agentic_note_agent::engine::StageContext;
use agentic_note_agent::llm::{anthropic::AnthropicProvider, openai::OpenAiProvider};
use agentic_note_agent::AgentHandler;
use agentic_note_core::Result;
use agentic_note_core::types::ParaCategory;
use agentic_note_test_utils::{MockLlmServer, TempVault};
use agentic_note_vault::Note;
use std::sync::Arc;

const OPENAI_CLASSIFICATION_RESPONSE: &str = r#"{
  "choices": [
    {
      "message": {
        "content": "{\"para\":\"projects\",\"tags\":[\"rust\",\"testing\"],\"confidence\":0.98}"
      }
    }
  ]
}"#;

const ANTHROPIC_DISTILLER_RESPONSE: &str = r#"{
  "content": [
    {
      "text": "{\"summary\":\"Short summary\",\"key_ideas\":[\"one\",\"two\",\"three\"]}"
    }
  ]
}"#;

#[tokio::test]
async fn para_classifier_uses_mock_openai_base_url() -> Result<()> {
    let server = MockLlmServer::start_openai(OPENAI_CLASSIFICATION_RESPONSE).await?;
    let vault = TempVault::new()?;
    let note = Note::create(
        vault.path(),
        "Sprint plan",
        ParaCategory::Inbox,
        "Need roadmap and execution details",
        vec!["planning".into()],
    )?;
    let mut ctx = StageContext::from_note(&note, vault.path());
    let agent = ParaClassifier::new(Arc::new(OpenAiProvider::new_custom(
        format!("{}/v1", server.base_url()),
        "test-key",
        "gpt-4o-mini",
    )));

    let result = agent.execute(&mut ctx, &toml::Value::Table(Default::default())).await?;

    assert_eq!(result["para"], "projects");
    assert_eq!(result["tags"][0], "rust");
    Ok(())
}

#[tokio::test]
async fn distiller_uses_mock_anthropic_base_url() -> Result<()> {
    let server = MockLlmServer::start_anthropic(ANTHROPIC_DISTILLER_RESPONSE).await?;
    let vault = TempVault::new()?;
    let note = Note::create(
        vault.path(),
        "Research note",
        ParaCategory::Resources,
        "Dense note body that needs condensing.",
        vec!["summary".into()],
    )?;
    let mut ctx = StageContext::from_note(&note, vault.path());
    let agent = Distiller::new(Arc::new(AnthropicProvider::new_custom(
        format!("{}/v1", server.base_url()),
        "test-key",
        "claude-3-5-sonnet",
    )));

    let result = agent.execute(&mut ctx, &toml::Value::Table(Default::default())).await?;

    assert_eq!(result["summary"], "Short summary");
    assert_eq!(result["key_ideas"][2], "three");
    Ok(())
}

#[cfg(feature = "live-tests")]
#[tokio::test]
async fn para_classifier_runs_against_live_openai() -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY is required for --features live-tests");
    let vault = TempVault::new()?;
    let note = Note::create(
        vault.path(),
        "Project kickoff",
        ParaCategory::Inbox,
        "Plan milestones, assign owners, and track deliverables for the Q2 launch.",
        vec!["planning".into(), "delivery".into()],
    )?;
    let mut ctx = StageContext::from_note(&note, vault.path());
    let agent = ParaClassifier::new(Arc::new(OpenAiProvider::new_openai(
        api_key,
        "gpt-4o-mini",
    )));

    let result = agent.execute(&mut ctx, &toml::Value::Table(Default::default())).await?;
    let para = result["para"]
        .as_str()
        .expect("para-classifier should return a string category");
    assert!(matches!(
        para,
        "projects" | "areas" | "resources" | "archives" | "inbox"
    ));
    Ok(())
}

#[cfg(feature = "live-tests")]
#[tokio::test]
async fn distiller_runs_against_live_anthropic() -> Result<()> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY is required for --features live-tests");
    let vault = TempVault::new()?;
    let note = Note::create(
        vault.path(),
        "Research digest",
        ParaCategory::Resources,
        "Rust integration tests should stay hermetic, deterministic, and cheap to rerun.",
        vec!["research".into()],
    )?;
    let mut ctx = StageContext::from_note(&note, vault.path());
    let agent = Distiller::new(Arc::new(AnthropicProvider::new(
        api_key,
        "claude-3-5-sonnet-latest",
    )));

    let result = agent.execute(&mut ctx, &toml::Value::Table(Default::default())).await?;
    let summary = result["summary"]
        .as_str()
        .expect("distiller should return a summary string");
    assert!(!summary.trim().is_empty());
    Ok(())
}
