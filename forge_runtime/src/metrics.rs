//! Runtime metrics for operations, timing, and cache statistics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Kind of metric being tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    /// Graph query operation
    GraphQuery,
    /// Semantic search operation
    Search,
    /// CFG analysis operation
    CfgAnalysis,
    /// Cache hit
    CacheHit,
    /// Cache miss
    CacheMiss,
    /// Re-index operation
    Reindex,
}

/// Runtime metrics collector.
///
/// Tracks operation counts, timing data, and cache statistics.
#[derive(Clone, Debug)]
pub struct RuntimeMetrics {
    inner: Arc<MetricsInner>,
}

#[derive(Debug)]
struct MetricsInner {
    /// Operation counts by kind
    counts: [AtomicU64; 6],
    /// Total operation time (nanoseconds)
    total_time_ns: AtomicU64,
    /// Cache hits
    cache_hits: AtomicU64,
    /// Cache misses
    cache_misses: AtomicU64,
}

impl RuntimeMetrics {
    /// Creates a new metrics collector.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                counts: [
                    AtomicU64::new(0),  // GraphQuery
                    AtomicU64::new(0),  // Search
                    AtomicU64::new(0),  // CfgAnalysis
                    AtomicU64::new(0),  // CacheHit
                    AtomicU64::new(0),  // CacheMiss
                    AtomicU64::new(0),  // Reindex
                ],
                total_time_ns: AtomicU64::new(0),
                cache_hits: AtomicU64::new(0),
                cache_misses: AtomicU64::new(0),
            }),
        }
    }

    /// Records a metric occurrence.
    pub fn record(&self, kind: MetricKind) {
        let index = kind as usize;
        self.inner.counts[index].fetch_add(1, Ordering::Relaxed);
    }

    /// Records a timed operation.
    pub fn record_timing(&self, kind: MetricKind, duration: Duration) {
        self.record(kind);
        self.inner.total_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Records a cache access.
    pub fn record_cache_access(&self, hit: bool) {
        if hit {
            self.inner.cache_hits.fetch_add(1, Ordering::Relaxed);
            self.record(MetricKind::CacheHit);
        } else {
            self.inner.cache_misses.fetch_add(1, Ordering::Relaxed);
            self.record(MetricKind::CacheMiss);
        }
    }

    /// Gets the count for a specific metric.
    pub fn count(&self, kind: MetricKind) -> u64 {
        self.inner.counts[kind as usize].load(Ordering::Relaxed)
    }

    /// Gets the total operation time.
    pub fn total_time(&self) -> Duration {
        Duration::from_nanos(self.inner.total_time_ns.load(Ordering::Relaxed))
    }

    /// Gets the cache hit rate (0.0 to 1.0).
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.inner.cache_hits.load(Ordering::Relaxed);
        let misses = self.inner.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f64 / total as f64
    }

    /// Gets all metrics as a summary.
    pub fn summary(&self) -> MetricsSummary {
        MetricsSummary {
            graph_queries: self.count(MetricKind::GraphQuery),
            searches: self.count(MetricKind::Search),
            cfg_analyses: self.count(MetricKind::CfgAnalysis),
            reindex_ops: self.count(MetricKind::Reindex),
            total_time: self.total_time(),
            cache_hit_rate: self.cache_hit_rate(),
        }
    }

    /// Resets all metrics to zero.
    pub fn reset(&self) {
        for count in &self.inner.counts {
            count.store(0, Ordering::Relaxed);
        }
        self.inner.total_time_ns.store(0, Ordering::Relaxed);
        self.inner.cache_hits.store(0, Ordering::Relaxed);
        self.inner.cache_misses.store(0, Ordering::Relaxed);
    }
}

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of runtime metrics.
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// Number of graph queries performed
    pub graph_queries: u64,
    /// Number of search operations performed
    pub searches: u64,
    /// Number of CFG analyses performed
    pub cfg_analyses: u64,
    /// Number of re-index operations performed
    pub reindex_ops: u64,
    /// Total time spent on operations
    pub total_time: Duration,
    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_record() {
        let metrics = RuntimeMetrics::new();

        metrics.record(MetricKind::GraphQuery);
        metrics.record(MetricKind::GraphQuery);

        assert_eq!(metrics.count(MetricKind::GraphQuery), 2);
        assert_eq!(metrics.count(MetricKind::Search), 0);
    }

    #[test]
    fn test_metrics_timing() {
        let metrics = RuntimeMetrics::new();

        metrics.record_timing(MetricKind::Search, Duration::from_millis(100));

        assert_eq!(metrics.count(MetricKind::Search), 1);
        assert_eq!(metrics.total_time(), Duration::from_millis(100));
    }

    #[test]
    fn test_cache_hit_rate() {
        let metrics = RuntimeMetrics::new();

        metrics.record_cache_access(true);
        metrics.record_cache_access(true);
        metrics.record_cache_access(false);

        // 2 hits out of 3 = 0.666...
        assert!((metrics.cache_hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = RuntimeMetrics::new();

        metrics.record(MetricKind::GraphQuery);
        metrics.record_cache_access(true);

        metrics.reset();

        assert_eq!(metrics.count(MetricKind::GraphQuery), 0);
        assert_eq!(metrics.cache_hit_rate(), 0.0);
    }

    #[test]
    fn test_metrics_summary() {
        let metrics = RuntimeMetrics::new();

        metrics.record(MetricKind::GraphQuery);
        metrics.record(MetricKind::Search);
        metrics.record(MetricKind::CfgAnalysis);
        metrics.record_cache_access(true);

        let summary = metrics.summary();

        assert_eq!(summary.graph_queries, 1);
        assert_eq!(summary.searches, 1);
        assert_eq!(summary.cfg_analyses, 1);
        assert_eq!(summary.cache_hit_rate, 1.0);
    }
}
