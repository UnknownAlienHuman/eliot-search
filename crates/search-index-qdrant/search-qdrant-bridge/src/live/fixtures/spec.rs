//! Frozen qualification collection, vector, payload and identity constants.

/// Disposable qualification collection. The server itself is disposable, so a
/// fixed name is collision-free by construction.
pub const QUALIFICATION_COLLECTION: &str = "t22_qual_probe";
/// Qualified sparse vector names (frozen lexical legs).
pub const VECTOR_CODE: &str = "lex_code_v1";
/// Neutral-text sparse vector name.
pub const VECTOR_TEXT: &str = "lex_text_neutral_v1";
/// Tenant payload field.
pub const FIELD_TENANT: &str = "tenant";
/// Access-partition payload field.
pub const FIELD_ACCESS: &str = "access_partition";
/// Inclusive valid-from epoch payload field.
pub const FIELD_FROM: &str = "valid_from_epoch";
/// Exclusive valid-until epoch payload field.
pub const FIELD_UNTIL: &str = "valid_until_epoch_exclusive";
/// Payload field that is ingested but deliberately never indexed: the strict
/// negative fixture.
pub const FIELD_UNINDEXED: &str = "unit_kind";
/// Eligible tenant population.
pub const TENANT_A: &str = "tenant-a";
/// Ineligible tenant population.
pub const TENANT_B: &str = "tenant-b";
/// Eligible access partition.
pub const ACCESS_A: &str = "partition-a";
/// Visible epoch shared by retrieval and IDF corpus plans.
pub const VISIBLE_EPOCH: u64 = 42;
/// Same epoch as `i64` for integer payload/range construction without casts.
pub const VISIBLE_EPOCH_I64: i64 = 42;
/// Exactly representable negative signed-epoch fixture.
pub const EPOCH_MIN: i64 = -9_007_199_254_740_000;
/// Exactly representable positive signed-epoch fixture.
pub const EPOCH_MAX: i64 = 9_007_199_254_740_000;
/// UUID point proving UUID transport without the client `uuid` feature.
pub const UUID_POINT: &str = "550e8400-e29b-41d4-a716-446655440000";
