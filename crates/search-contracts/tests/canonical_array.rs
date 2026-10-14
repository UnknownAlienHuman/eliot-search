//! #396 causal array-row fixtures. `AUTHORED_NOT_RUN` under #352.

use std::cell::Cell;

use search_contracts::{
    BoundedBytes, BoundedList, BoundedMap, CanonicalDigestDomain, CanonicalKey, CanonicalText,
    CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit, MAX_CANONICAL_BYTES,
    MAX_CANONICAL_DEPTH, MAX_COLLECTION_ITEMS, blake3_canonical, blake3_canonical_array,
    to_canonical_cbor,
};

fn cbor_domain() -> CanonicalDigestDomain {
    CanonicalDigestDomain::parse("eliot/cbor/array-fixture/v1").expect("domain")
}

fn limit() -> DigestInputLimit {
    DigestInputLimit::new(MAX_CANONICAL_BYTES).expect("limit")
}

fn text(value: &str) -> CanonicalValue {
    CanonicalValue::Text(CanonicalText::new(value).expect("text"))
}

fn object(entries: Vec<(&str, CanonicalValue)>) -> CanonicalValue {
    CanonicalValue::Object(
        BoundedMap::from_entries(
            entries
                .into_iter()
                .map(|(key, value)| (CanonicalKey::new(key).expect("key"), value)),
        )
        .expect("object"),
    )
}

fn array(rows: Vec<CanonicalValue>) -> CanonicalValue {
    CanonicalValue::Array(BoundedList::new(rows).expect("rows"))
}

fn nested(depth: usize) -> CanonicalValue {
    let mut value = CanonicalValue::Bool(true);
    for _ in 0..depth {
        value = object(vec![("k", value)]);
    }
    value
}

// From the worker's partial fixture: instrument finite and infinite generators.
struct CountingRows {
    rows: std::vec::IntoIter<CanonicalValue>,
    calls: usize,
}

impl CountingRows {
    fn new(rows: Vec<CanonicalValue>) -> Self {
        Self {
            rows: rows.into_iter(),
            calls: 0,
        }
    }
}

impl Iterator for CountingRows {
    type Item = Result<CanonicalValue, ContractError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.calls += 1;
        self.rows.next().map(Ok)
    }
}

struct InfiniteRows {
    calls: usize,
}

impl Iterator for InfiniteRows {
    type Item = Result<CanonicalValue, ContractError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.calls += 1;
        Some(Ok(CanonicalValue::Bool(true)))
    }
}

struct ForbiddenConversion<'a>(&'a Cell<usize>);

impl IntoIterator for ForbiddenConversion<'_> {
    type Item = Result<CanonicalValue, ContractError>;
    type IntoIter = std::iter::Empty<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.set(self.0.get() + 1);
        panic!("rejection must precede iterator conversion")
    }
}

#[test]
fn empty_and_mixed_rows_keep_existing_array_digest_and_order() {
    let domain = cbor_domain();
    let rows = vec![
        CanonicalValue::Null,
        CanonicalValue::I64(-24),
        text("é"),
        CanonicalValue::Bytes(BoundedBytes::new(vec![0, 255]).expect("bytes")),
        object(vec![("aa", CanonicalValue::U64(24)), ("z", text("nested"))]),
        array(vec![CanonicalValue::Bool(false)]),
    ];
    for values in [vec![], rows.clone()] {
        assert_eq!(
            blake3_canonical_array(
                &domain,
                values.len(),
                values.clone().into_iter().map(Ok),
                limit()
            ),
            blake3_canonical(&domain, &array(values), limit()),
        );
    }
    let expected = blake3_canonical_array(
        &domain,
        rows.len(),
        rows.clone().into_iter().map(Ok),
        limit(),
    )
    .unwrap();
    let mut reversed = rows;
    reversed.reverse();
    assert_ne!(
        expected,
        blake3_canonical_array(
            &domain,
            reversed.len(),
            reversed.into_iter().map(Ok),
            limit()
        )
        .unwrap()
    );
}

#[test]
fn maximum_declared_count_is_admitted_and_consumed_exactly() {
    let mut rows = CountingRows::new(vec![CanonicalValue::Null; MAX_COLLECTION_ITEMS]);
    assert!(
        blake3_canonical_array(&cbor_domain(), MAX_COLLECTION_ITEMS, &mut rows, limit()).is_ok()
    );
    assert_eq!(rows.calls, MAX_COLLECTION_ITEMS + 1);
}

#[test]
fn over_limit_count_rejects_before_iterator_conversion() {
    let converted = Cell::new(0);
    let error = blake3_canonical_array(
        &cbor_domain(),
        MAX_COLLECTION_ITEMS + 1,
        ForbiddenConversion(&converted),
        limit(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ContractErrorKind::TooManyItems);
    assert_eq!(error.field(), "canonical_array_length");
    assert_eq!(converted.get(), 0);
}

#[test]
fn raw_domain_and_header_budget_reject_before_iterator_conversion() {
    let converted = Cell::new(0);
    let raw = CanonicalDigestDomain::parse("eliot/raw/array-fixture/v1").unwrap();
    let error =
        blake3_canonical_array(&raw, 0, ForbiddenConversion(&converted), limit()).unwrap_err();
    assert_eq!(error.kind(), ContractErrorKind::InvalidTaggedVariant);
    let domain = cbor_domain();
    let framing = domain.as_str().len() + 1;
    let error = blake3_canonical_array(
        &domain,
        0,
        ForbiddenConversion(&converted),
        DigestInputLimit::new(framing).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
    assert_eq!(converted.get(), 0);
    assert!(
        blake3_canonical_array(
            &domain,
            0,
            std::iter::empty(),
            DigestInputLimit::new(framing + 1).unwrap()
        )
        .is_ok()
    );
}

#[test]
fn short_stream_returns_length_error_without_excess_probe() {
    let mut rows = CountingRows::new(vec![CanonicalValue::Null]);
    let error = blake3_canonical_array(&cbor_domain(), 2, &mut rows, limit()).unwrap_err();
    assert_eq!(error, ContractError::malformed("canonical_array_length"));
    assert_eq!(rows.calls, 2);
}

#[test]
fn extra_and_infinite_streams_consume_only_one_excess_row() {
    let mut finite = CountingRows::new(vec![CanonicalValue::Null; 2]);
    assert_eq!(
        blake3_canonical_array(&cbor_domain(), 1, &mut finite, limit()),
        Err(ContractError::malformed("canonical_array_length"))
    );
    assert_eq!(finite.calls, 2);
    for count in [0, 3] {
        let mut infinite = InfiniteRows { calls: 0 };
        assert_eq!(
            blake3_canonical_array(&cbor_domain(), count, &mut infinite, limit()),
            Err(ContractError::malformed("canonical_array_length"))
        );
        assert_eq!(infinite.calls, count + 1);
    }
}

#[test]
fn caller_error_propagates_but_an_error_row_beyond_n_is_excess() {
    let failure = ContractError::malformed("fixture_producer");
    let mut calls = 0;
    let rows = std::iter::from_fn(|| {
        calls += 1;
        match calls {
            1 => Some(Ok(CanonicalValue::Null)),
            2 => Some(Err(failure.clone())),
            _ => panic!("producer error must stop consumption"),
        }
    });
    assert_eq!(
        blake3_canonical_array(&cbor_domain(), 2, rows, limit()),
        Err(failure.clone())
    );
    assert_eq!(calls, 2);
    assert_eq!(
        blake3_canonical_array(
            &cbor_domain(),
            1,
            [Ok(CanonicalValue::Null), Err(failure)],
            limit()
        ),
        Err(ContractError::malformed("canonical_array_length"))
    );
}

#[test]
fn aggregate_budget_includes_domain_header_and_all_rows() {
    let domain = cbor_domain();
    let rows = vec![text("abc"), CanonicalValue::U64(24)];
    let bytes = to_canonical_cbor(&array(rows.clone())).unwrap();
    assert_eq!(
        bytes.as_slice(),
        &[0x82, 0x63, b'a', b'b', b'c', 0x18, 0x18]
    );
    let exact = domain.as_str().len() + 1 + bytes.len();
    assert_eq!(
        blake3_canonical_array(
            &domain,
            2,
            rows.clone().into_iter().map(Ok),
            DigestInputLimit::new(exact).unwrap()
        ),
        blake3_canonical(
            &domain,
            &array(rows.clone()),
            DigestInputLimit::new(exact).unwrap()
        ),
    );
    let error = blake3_canonical_array(
        &domain,
        2,
        rows.into_iter().map(Ok),
        DigestInputLimit::new(exact - 1).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
}

#[test]
fn enclosing_array_participates_in_depth_ceiling() {
    let domain = cbor_domain();
    let accepted = nested(MAX_CANONICAL_DEPTH - 1);
    assert_eq!(
        blake3_canonical_array(&domain, 1, [Ok(accepted.clone())], limit()),
        blake3_canonical(&domain, &array(vec![accepted]), limit())
    );
    let rejected = nested(MAX_CANONICAL_DEPTH);
    assert!(rejected.validate().is_ok());
    let error = blake3_canonical_array(&domain, 1, [Ok(rejected.clone())], limit()).unwrap_err();
    assert_eq!(error.kind(), ContractErrorKind::DepthExceeded);
    assert_eq!(
        Err(error),
        blake3_canonical(&domain, &array(vec![rejected]), limit())
    );
}

#[test]
fn negative_signed_integer_is_valid_but_nonnegative_is_noncanonical() {
    assert!(
        blake3_canonical_array(&cbor_domain(), 1, [Ok(CanonicalValue::I64(-1))], limit()).is_ok()
    );
    for value in [0, 1] {
        let error =
            blake3_canonical_array(&cbor_domain(), 1, [Ok(CanonicalValue::I64(value))], limit())
                .unwrap_err();
        assert_eq!(error.kind(), ContractErrorKind::NonCanonical);
    }
}
