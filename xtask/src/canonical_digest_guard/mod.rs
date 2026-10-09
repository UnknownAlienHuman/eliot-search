//! Bounded, fail-closed source inventory for #237. This is a review guard,
//! not Rust type resolution, macro expansion or runtime algorithm proof.
//!
//! Every retained site is pinned to its full source hash and occurrence count.
//! Tests and archives are inspected and individually classified, never ignored.
//! New sites, changed source and unresolved compiler-required entries fail.

mod allowlist;
mod detector;
#[cfg(test)]
mod fixture;
mod inventory;
mod scan;
mod syntax;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Key {
    path: String,
    symbol: String,
    signal: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Site {
    key: Key,
    count: usize,
    test: bool,
    source_sha256: String,
}

impl Site {
    fn json(&self) -> Value {
        json!({"path":self.key.path,"symbol":self.key.symbol,
            "signal":self.key.signal,"count":self.count,"test":self.test,
            "source_sha256":self.source_sha256})
    }
}

/// Diagnostic inventory only; this does not create or accept exceptions.
///
/// # Errors
/// Returns an error on incomplete reads, unsupported syntax or exhausted bounds.
pub fn inventory(root: &Path) -> Result<Value, String> {
    let sites = inventory::collect(root)?;
    Ok(Value::Array(sites.values().map(Site::json).collect()))
}

/// Validate an exact, reviewed source ledger. No network or repository writes.
///
/// # Errors
/// Returns an error for inventory failures or missing, changed, stale or
/// unresolved ledger entries. Owner states are a reviewed offline snapshot.
pub fn validate(root: &Path) -> Result<(), String> {
    let sites = inventory::collect(root)?;
    allowlist::validate(root, &sites)
}

type Sites = BTreeMap<Key, Site>;
