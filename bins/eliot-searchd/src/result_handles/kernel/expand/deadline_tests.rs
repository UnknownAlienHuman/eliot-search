//! Synthetic read-completion/output-boundary faults, not native readback proof.

use super::*;
use super::super::model::ResultHandleRecord;
use std::collections::BTreeMap;
use std::time::Duration;

fn record(deadline: Instant) -> ResultHandleRecord {
    ResultHandleRecord {
        namespace_id: "fixture-namespace".to_owned(),
        session_tag: 7,
        source_fence_digest: "fixture-fence".to_owned(),
        source_id: "fixture-source".to_owned(),
        revision_id: "fixture-revision".to_owned(),
        content_digest: "fixture-content".to_owned(),
        byte_length: 4,
        expires_at: deadline,
    }
}

fn catalog(deadline: Instant) -> ResultHandleCatalog {
    ResultHandleCatalog {
        namespace_id: "fixture-namespace".to_owned(),
        session_tag: 7,
        entropy_poisoned: false,
        records: BTreeMap::from([
            ("target".to_owned(), record(deadline)),
            ("other".to_owned(), record(deadline + Duration::from_secs(60))),
        ]),
    }
}

fn expansion() -> ResultHandleExpansion {
    ResultHandleExpansion {
        source_handle: "target".to_owned(),
        byte_start: 0,
        byte_end: 4,
        source_byte_length: 4,
        bytes: b"data".to_vec(),
    }
}

#[test]
fn read_completion_at_expiry_returns_no_preparation_and_removes_only_target() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut catalog = catalog(deadline);
    assert!(matches!(
        PreparedExpansion::new(&mut catalog, expansion(), deadline, deadline),
        Err(ResultHandleError::Expired),
    ));
    assert!(!catalog.records.contains_key("target"));
    assert_eq!(catalog.records.len(), 1);
    assert!(catalog.records.contains_key("other"));
}

#[test]
fn a_record_swept_during_readback_cannot_return_bytes() {
    let before = Instant::now();
    let deadline = before + Duration::from_secs(60);
    let mut catalog = catalog(deadline);
    catalog.records.remove("target");
    assert!(matches!(
        PreparedExpansion::new(&mut catalog, expansion(), deadline, before),
        Err(ResultHandleError::Expired),
    ));
    assert_eq!(catalog.records.len(), 1);
}

#[test]
fn expiry_during_caller_diagnostics_never_enters_the_output_callback() {
    let before = Instant::now();
    let deadline = before + Duration::from_secs(60);
    let mut catalog = catalog(deadline);
    let prepared = PreparedExpansion::new(&mut catalog, expansion(), deadline, before)
        .unwrap_or_else(|_| panic!("valid synthetic preparation"));
    let result = prepared.deliver_at(deadline, |_, _| panic!("expired output callback"));
    assert_eq!(result, Err(ResultHandleError::Expired.code().to_owned()));
    assert!(!catalog.records.contains_key("target"));
    assert!(catalog.records.contains_key("other"));
}

#[test]
fn successful_delivery_preserves_original_deadline_and_retained_handle() {
    let before = Instant::now();
    let deadline = before + Duration::from_secs(60);
    let mut catalog = catalog(deadline);
    let prepared = PreparedExpansion::new(&mut catalog, expansion(), deadline, before)
        .unwrap_or_else(|_| panic!("valid synthetic preparation"));
    prepared.deliver_at(before, |result, observed_deadline| {
        assert_eq!(result.bytes, b"data");
        assert_eq!(result.source_handle, "target");
        assert_eq!(observed_deadline, deadline);
        Ok(())
    }).unwrap();
    assert_eq!(catalog.records["target"].expires_at, deadline);
    assert_eq!(catalog.records.len(), 2);
}

#[test]
fn abandoned_preparation_or_output_error_does_not_renew_or_delete_the_handle() {
    for output_error in [false, true] {
        let before = Instant::now();
        let deadline = before + Duration::from_secs(60);
        let mut catalog = catalog(deadline);
        let prepared = PreparedExpansion::new(&mut catalog, expansion(), deadline, before)
            .unwrap_or_else(|_| panic!("valid synthetic preparation"));
        if output_error {
            assert_eq!(
                prepared.deliver_at(before, |_, _| Err("output-failed".to_owned())),
                Err("output-failed".to_owned()),
            );
        } else {
            drop(prepared);
        }
        assert_eq!(catalog.records["target"].expires_at, deadline);
        assert_eq!(catalog.records.len(), 2);
    }
}
