use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::*;
use crate::MaterializationError;

fn limits() -> MaterializationLimits {
    MaterializationLimits {
        max_input_bytes: 1024,
        max_output_bytes: 1024,
        max_lines: 64,
    }
}

#[test]
fn unchanged_unicode_reuses_the_input_allocation_and_exact_lines() {
    let bytes = "α\r\nβ\nγ\rδ".as_bytes().to_vec();
    let pointer = bytes.as_ptr();
    let product = materialize_utf8(
        bytes,
        limits(),
        Instant::now() + Duration::from_secs(1),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(product.text().as_ptr(), pointer);
    assert_eq!(product.text(), "α\r\nβ\nγ\rδ");
    assert_eq!(
        product
            .lines()
            .iter()
            .map(|line| (line.source_start, line.content_end, line.source_end))
            .collect::<Vec<_>>(),
        [(0, 2, 4), (4, 6, 7), (7, 9, 10), (10, 12, 12)]
    );
}

#[test]
fn empty_and_final_terminator_do_not_manufacture_lines() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let cancel = AtomicBool::new(false);
    let empty = materialize_utf8(Vec::new(), limits(), deadline, &cancel).unwrap();
    assert!(empty.text().is_empty());
    assert!(empty.lines().is_empty());
    let terminated = materialize_utf8(b"a\n".to_vec(), limits(), deadline, &cancel).unwrap();
    assert_eq!(terminated.lines().len(), 1);
}

#[test]
fn content_refusals_remain_distinct() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let cancel = AtomicBool::new(false);
    assert_eq!(
        materialize_utf8(b"a\0b".to_vec(), limits(), deadline, &cancel),
        Err(MaterializationError::BinaryContent)
    );
    assert_eq!(
        materialize_utf8(vec![0xff], limits(), deadline, &cancel),
        Err(MaterializationError::InvalidUtf8)
    );
}

#[test]
fn execution_limits_and_cancellation_are_not_content_refusals() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut caps = limits();
    caps.max_lines = 1;
    assert_eq!(
        materialize_utf8(b"a\nb".to_vec(), caps, deadline, &AtomicBool::new(false)),
        Err(MaterializationError::TooManyLines)
    );
    assert_eq!(
        materialize_utf8(Vec::new(), limits(), deadline, &AtomicBool::new(true)),
        Err(MaterializationError::Cancelled)
    );
    assert_eq!(
        materialize_utf8(
            Vec::new(),
            limits(),
            Instant::now(),
            &AtomicBool::new(false)
        ),
        Err(MaterializationError::Deadline)
    );
}
