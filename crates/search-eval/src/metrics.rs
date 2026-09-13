//! Deterministic metric scoring, aggregation, comparison, SLOs, and resources.
//!
//! Attempt scoring, denominator-preserving aggregation, preregistered A/B/C
//! comparison, SLO evaluation and warm-up/measured resource accounting have
//! separate private owners behind this stable public facade.

mod aggregate;
mod compare;
mod resource;
mod score;
mod slo;
mod support;

pub use aggregate::{AggregatedMetric, BaselineMetricReport, aggregate_block};
pub use compare::{
    BaselineComparison, BaselineComparisonClass, MetricDelta, MetricGates, compare_abc,
};
pub use resource::{
    ResourceLane, ResourceReport, compute_resource_report, compute_resource_report_for_lane,
};
pub use score::{
    CaseMetricSet, CaseMetricValue, MetricObservation, MetricObservationState, score_case,
};
pub use slo::{
    SloDefinition, SloDirection, SloOutcome, SloReport, SloStatus,
    evaluate_candidate_slos,
};
