//! CLI command for displaying metrics summary.
//!
//! When a MetricsHandle is available, encodes real Prometheus metrics.
//! Otherwise displays a hint about enabling the metrics server.

use crate::metrics_handle::MetricsHandle;

/// Display current metrics as a formatted CLI table.
pub fn show_metrics() -> anyhow::Result<()> {
    println!("Metrics Summary");
    println!("{}", "=".repeat(50));
    println!();
    println!("Note: Enable metrics server with [metrics] enabled = true in config.");
    println!("Then scrape http://127.0.0.1:9091/metrics for Prometheus format.");
    println!();
    println!("{}", "-".repeat(50));
    println!("  pipeline_execution_duration_seconds (histogram)");
    println!("  search_query_duration_seconds       (histogram)");
    println!("  sync_duration_seconds               (histogram)");
    println!("  notes_total                         (gauge)");
    println!("  llm_requests_total                  (counter)");
    println!("  llm_cache_hits_total                (counter)");
    println!("  review_queue_pending                (gauge)");
    println!("{}", "-".repeat(50));
    Ok(())
}

/// Display metrics from a live MetricsHandle (OpenMetrics text format).
pub fn show_metrics_live(handle: &MetricsHandle) -> anyhow::Result<()> {
    let text = handle.encode();
    if text.trim().is_empty() {
        println!("No metrics recorded yet.");
    } else {
        println!("{text}");
    }
    Ok(())
}
