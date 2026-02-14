use clap::Subcommand;
use std::path::Path;

use agentic_note_agent::engine::PipelineScheduler;
use crate::output::{print_json, OutputFormat};

#[derive(Subcommand)]
pub enum PipelineCmd {
    /// Show active pipeline triggers and schedules
    Status,
}

pub fn run(cmd: PipelineCmd, vault_path: &Path, fmt: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        PipelineCmd::Status => {
            let pipelines_dir = vault_path.join("pipelines");
            let mut scheduler = PipelineScheduler::new();
            let count = scheduler.scan_and_register(&pipelines_dir).unwrap_or(0);

            let schedules = scheduler.list_schedules();

            match fmt {
                OutputFormat::Json => {
                    let items: Vec<_> = schedules.iter().map(|s| {
                        serde_json::json!({
                            "pipeline": s.pipeline_name,
                            "trigger_type": format!("{:?}", s.trigger_type),
                            "cron": s.cron_expr,
                            "watch_path": s.watch_path,
                        })
                    }).collect();
                    print_json(&serde_json::json!({
                        "active_schedules": items,
                        "total": count,
                    }));
                }
                OutputFormat::Human => {
                    if schedules.is_empty() {
                        println!("No active pipeline schedules.");
                        println!("Add trigger.type = \"cron\" or \"watch\" to pipeline TOML files.");
                    } else {
                        println!("Active Pipeline Schedules ({count}):");
                        println!("{:<20} {:<8} {}", "Pipeline", "Type", "Config");
                        println!("{}", "-".repeat(50));
                        for s in schedules {
                            let config = match (&s.cron_expr, &s.watch_path) {
                                (Some(c), _) => format!("cron: {c}"),
                                (_, Some(p)) => format!("watch: {p}"),
                                _ => "\u{2014}".to_string(),
                            };
                            println!("{:<20} {:<8} {}", s.pipeline_name, format!("{:?}", s.trigger_type), config);
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
