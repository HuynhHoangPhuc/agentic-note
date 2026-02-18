//! Prometheus metrics handle with registered metric families.
//!
//! Holds the prometheus-client registry and all metric instances.
//! Passed to subsystems for recording; served via HTTP on /metrics.

use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{exponential_buckets, Histogram};
use prometheus_client::registry::Registry;
use std::sync::{Arc, Mutex};

/// Label set for pipeline metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct PipelineLabels {
    pub pipeline: String,
    pub status: String,
}

/// Label set for LLM request metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct LlmLabels {
    pub provider: String,
    pub status: String,
}

/// Label set for search metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct SearchLabels {
    pub mode: String,
}

/// Label set for sync metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct SyncLabels {
    pub peer_id: String,
    pub status: String,
}

/// A cloneable histogram constructor that holds shared bucket boundaries.
#[derive(Clone)]
struct BucketedHistogram(Arc<Vec<f64>>);

impl BucketedHistogram {
    fn new(buckets: impl Iterator<Item = f64>) -> Self {
        Self(Arc::new(buckets.collect()))
    }

    fn make(&self) -> Histogram {
        Histogram::new(self.0.iter().copied())
    }
}

/// Shared metrics handle holding all Prometheus metric instances.
#[derive(Clone)]
pub struct MetricsHandle {
    pub registry: Arc<Mutex<Registry>>,
    pub pipeline_duration: Family<PipelineLabels, Histogram, BucketedHistogram>,
    pub search_duration: Family<SearchLabels, Histogram, BucketedHistogram>,
    pub sync_duration: Family<SyncLabels, Histogram, BucketedHistogram>,
    pub notes_total: Gauge,
    pub llm_requests: Family<LlmLabels, Counter>,
    pub llm_cache_hits: Counter,
    pub review_pending: Gauge,
}

impl prometheus_client::metrics::family::MetricConstructor<Histogram> for BucketedHistogram {
    fn new_metric(&self) -> Histogram {
        self.make()
    }
}

impl MetricsHandle {
    /// Create a new MetricsHandle with all metrics registered.
    pub fn new() -> Self {
        let mut registry = Registry::default();

        let pipeline_ctor =
            BucketedHistogram::new(exponential_buckets(0.001, 2.0, 15));
        let pipeline_duration =
            Family::<PipelineLabels, Histogram, BucketedHistogram>::new_with_constructor(
                pipeline_ctor,
            );
        registry.register(
            "pipeline_execution_duration_seconds",
            "Pipeline execution duration",
            pipeline_duration.clone(),
        );

        let search_ctor =
            BucketedHistogram::new(exponential_buckets(0.0001, 2.0, 12));
        let search_duration =
            Family::<SearchLabels, Histogram, BucketedHistogram>::new_with_constructor(
                search_ctor,
            );
        registry.register(
            "search_query_duration_seconds",
            "Search query duration",
            search_duration.clone(),
        );

        let sync_ctor =
            BucketedHistogram::new(exponential_buckets(0.01, 2.0, 14));
        let sync_duration =
            Family::<SyncLabels, Histogram, BucketedHistogram>::new_with_constructor(
                sync_ctor,
            );
        registry.register(
            "sync_duration_seconds",
            "Sync operation duration",
            sync_duration.clone(),
        );

        let notes_total = Gauge::default();
        registry.register("notes_total", "Total notes in vault", notes_total.clone());

        let llm_requests = Family::<LlmLabels, Counter>::default();
        registry.register(
            "llm_requests_total",
            "Total LLM API requests",
            llm_requests.clone(),
        );

        let llm_cache_hits = Counter::default();
        registry.register(
            "llm_cache_hits_total",
            "LLM cache hit count",
            llm_cache_hits.clone(),
        );

        let review_pending = Gauge::default();
        registry.register(
            "review_queue_pending",
            "Pending review items",
            review_pending.clone(),
        );

        Self {
            registry: Arc::new(Mutex::new(registry)),
            pipeline_duration,
            search_duration,
            sync_duration,
            notes_total,
            llm_requests,
            llm_cache_hits,
            review_pending,
        }
    }

    /// Encode all metrics as OpenMetrics text format.
    pub fn encode(&self) -> String {
        let registry = match self.registry.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("metrics registry lock poisoned; recovering");
                poisoned.into_inner()
            }
        };
        let mut buf = String::new();
        if let Err(err) = encode(&mut buf, &registry) {
            tracing::warn!(error = %err, "encode metrics failed");
        }
        buf
    }
}

impl Default for MetricsHandle {
    fn default() -> Self {
        Self::new()
    }
}
