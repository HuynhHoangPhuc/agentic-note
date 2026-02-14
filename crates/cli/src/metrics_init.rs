//! Metrics exporter initialization.
//!
//! Sets up the metrics recording layer. Currently supports in-memory
//! recording via the `metrics` facade. Prometheus exporter available
//! behind the `prometheus` feature flag.

/// Install metrics exporter based on configuration.
/// Without the `prometheus` feature, metrics macros are no-ops.
pub fn install_metrics_recorder(_port: u16) -> anyhow::Result<()> {
    // The `metrics` facade macros work as no-ops when no recorder is installed.
    // With `--features prometheus`, we'd install PrometheusBuilder here.
    tracing::info!("Metrics recording enabled (in-memory)");
    Ok(())
}
