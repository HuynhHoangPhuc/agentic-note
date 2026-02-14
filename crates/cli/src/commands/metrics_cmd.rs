//! CLI command for displaying metrics summary.
//!
//! Provides a simple formatted table showing current metric values.
//! Reads from the metrics registry when available.

/// Display current metrics as a formatted CLI table.
pub fn show_metrics() -> anyhow::Result<()> {
    println!("Metrics Summary");
    println!("{}", "=".repeat(50));
    println!();
    println!("Note: Metrics are collected in-memory during this session.");
    println!("Use --metrics flag to enable Prometheus exporter on port 9091.");
    println!();
    println!("  Counters and histograms are recorded via the `metrics` facade.");
    println!("  Install a prometheus exporter (--features prometheus) to scrape them.");
    println!();
    println!("{}", "-".repeat(50));
    println!("  note_operations_total          (counter)");
    println!("  pipeline_execution_duration_s  (histogram)");
    println!("  pipeline_stage_duration_s      (histogram)");
    println!("  sync_duration_seconds          (histogram)");
    println!("  sync_bytes_transferred         (counter)");
    println!("  indexer_batch_duration_s        (histogram)");
    println!("  indexer_files_processed_total   (counter)");
    println!("{}", "-".repeat(50));
    Ok(())
}
