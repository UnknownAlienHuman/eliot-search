//! Closed qualification-fixture vocabulary and private vendor translation.
//!
//! Public constants stay stable for package-local live tests. Filter, payload
//! and point construction remain private to the bridge qualification harness.

mod filter;
mod points;
mod spec;

pub use spec::{
    ACCESS_A, EPOCH_MAX, EPOCH_MIN, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT,
    FIELD_UNINDEXED, FIELD_UNTIL, QUALIFICATION_COLLECTION, TENANT_A, TENANT_B,
    UUID_POINT, VECTOR_CODE, VECTOR_TEXT, VISIBLE_EPOCH, VISIBLE_EPOCH_I64,
};

pub(super) use filter::{
    base_eligibility, base_filter, exact_f64, keyword_condition,
    range_condition,
};
pub(super) use points::{
    int_value, num_point_id, point, snapshot_id, sparse_named, string_value,
    strong_ordering, update_completed,
};
