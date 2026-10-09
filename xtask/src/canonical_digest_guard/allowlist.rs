use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::{Key, Sites, scan};

const LEDGER: &str = "xtask/canonical-digest-ledger.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Classification {
    ProductionCompute,
    WireStoreDecode,
    AcceptedAlgorithm,
    SemanticFingerprint,
    NoncryptoFingerprint,
    TestVector,
    OptionalHarness,
    ArchiveHistory,
    CompilerRequired,
    OpaqueValueUse,
    TypedSchemaEncoding,
    NonDigestSurface,
    DependencyEdge,
}

impl Classification {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "production-compute" => Ok(Self::ProductionCompute),
            "wire-store-decode" => Ok(Self::WireStoreDecode),
            "accepted-algorithm" => Ok(Self::AcceptedAlgorithm),
            "semantic-fingerprint" => Ok(Self::SemanticFingerprint),
            "noncrypto-fingerprint" => Ok(Self::NoncryptoFingerprint),
            "test-vector" => Ok(Self::TestVector),
            "optional-harness" => Ok(Self::OptionalHarness),
            "archive-history" => Ok(Self::ArchiveHistory),
            "opaque-value-use" => Ok(Self::OpaqueValueUse),
            "typed-schema-encoding" => Ok(Self::TypedSchemaEncoding),
            "non-digest-surface" => Ok(Self::NonDigestSurface),
            "dependency-edge" => Ok(Self::DependencyEdge),
            "COMPILER_REQUIRED" => Ok(Self::CompilerRequired),
            _ => Err(format!("unknown classification {value}")),
        }
    }
}

pub(super) fn validate(root: &Path, sites: &Sites) -> Result<(), String> {
    // Review reasons and owner metadata have their own finite physical bound.
    let source = scan::read_bounded(
        &root.join(LEDGER),
        &mut scan::Budget::default(),
        8 * 1024 * 1024,
    )?;
    let ledger: Value = serde_json::from_str(&source).map_err(|e| format!("ledger: {e}"))?;
    validate_value(&ledger, sites)
}

pub(super) fn validate_value(ledger: &Value, sites: &Sites) -> Result<(), String> {
    fields(ledger, &["version", "owners", "sites"])?;
    if ledger.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("unsupported ledger version".into());
    }
    let entries = ledger
        .get("sites")
        .and_then(Value::as_array)
        .ok_or("ledger sites missing")?;
    if entries.len() > 32_768 {
        return Err("ledger entry limit exceeded".into());
    }
    let owners = ledger
        .get("owners")
        .and_then(Value::as_object)
        .ok_or("ledger owner snapshot missing")?;
    let mut seen = BTreeSet::new();
    let mut failures = Vec::new();
    for entry in entries {
        fields(
            entry,
            &[
                "path",
                "symbol",
                "signal",
                "count",
                "test",
                "source_sha256",
                "classification",
                "owner_issue",
                "phase",
                "reason",
            ],
        )?;
        let key = Key {
            path: text(entry, "path")?.into(),
            symbol: text(entry, "symbol")?.into(),
            signal: text(entry, "signal")?.into(),
        };
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate ledger key {key:?}"));
        }
        let Some(site) = sites.get(&key) else {
            failures.push(format!("stale ledger key {key:?}"));
            continue;
        };
        if text(entry, "source_sha256")? != site.source_sha256
            || entry.get("count").and_then(Value::as_u64) != u64::try_from(site.count).ok()
            || entry.get("test").and_then(Value::as_bool) != Some(site.test)
        {
            failures.push(format!("changed source/count/context: {key:?}"));
        }
        let class = Classification::parse(text(entry, "classification")?)?;
        let owner = entry
            .get("owner_issue")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0)
            .ok_or("invalid owner issue")?;
        let owner_key = owner.to_string();
        let owner_record = owners
            .get(&owner_key)
            .ok_or_else(|| format!("unverified owner #{owner}"))?;
        let phase = text(entry, "phase")?;
        if !matches!(
            phase,
            "accepted"
                | "owner-migration-new-profile"
                | "owner-migration-preserve-bytes"
                | "owner-removal"
        ) {
            return Err(format!("invalid phase: {phase}"));
        }
        if phase != "accepted" && owner_record.get("state").and_then(Value::as_str) != Some("OPEN")
        {
            failures.push(format!(
                "migration owner #{owner} is not open in reviewed snapshot"
            ));
        }
        if text(entry, "reason")?.len() < 16 {
            return Err(format!("missing narrow review reason {key:?}"));
        }
        if class == Classification::CompilerRequired {
            failures.push(format!("unresolved COMPILER_REQUIRED: {key:?}"));
        }
        if class == Classification::TestVector && !site.test {
            failures.push(format!("production site labelled test: {key:?}"));
        }
    }
    for key in sites.keys().filter(|key| !seen.contains(*key)) {
        failures.push(format!("unrecorded: {key:?}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("ledger missing {field}"))
}

fn fields(value: &Value, expected: &[&str]) -> Result<(), String> {
    let object = value.as_object().ok_or("ledger record is not an object")?;
    if object.len() != expected.len() || object.keys().any(|key| !expected.contains(&key.as_str()))
    {
        return Err("unknown or missing load-bearing ledger field".into());
    }
    Ok(())
}
