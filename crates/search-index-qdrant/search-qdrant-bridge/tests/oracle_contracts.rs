//! Public-API regressions for the in-memory reference bridge only.
//! No live server is started and no qualification evidence is produced.

#[path = "oracle_contracts/support.rs"]
mod support;
#[path = "oracle_contracts/mutation.rs"]
mod mutation;
#[path = "oracle_contracts/query.rs"]
mod query;
