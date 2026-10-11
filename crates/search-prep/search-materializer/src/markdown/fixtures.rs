//! Authored parser/profile cases; execution belongs to #215/#216.

use super::model::{HARD_LIMITS, InlineKind, MarkdownError, MarkdownKind};
use super::parse::parse_markdown;
use crate::request::CancellationToken;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

#[test]
fn raw_crlf_multibyte_nested_inventory_and_inert_metadata() {
    let input = "# Заголовок\r\n\r\n> цитата **сильная**\r\n>\r\n> - пункт [ссылка](https://example.invalid/never-fetch)\r\n\r\n<script>never_execute()</script>\r\n";
    let cancel = AtomicBool::new(false);
    let document = parse_markdown(
        input.as_bytes(),
        HARD_LIMITS,
        CancellationToken::new(&cancel),
        deadline(),
    )
    .unwrap();
    assert_eq!(document.source.as_bytes(), input.as_bytes());
    assert_eq!(document.nodes[0].span.start, 0);
    assert_eq!(document.nodes[0].span.end, input.len());
    assert!(
        document
            .nodes
            .iter()
            .any(|node| matches!(node.kind, MarkdownKind::Heading { level: 1, .. }))
    );
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.kind == MarkdownKind::Quote)
    );
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.kind == MarkdownKind::Item)
    );
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.kind == MarkdownKind::RawHtmlBlock)
    );
    for node in &document.nodes {
        assert!(input.get(node.span.start..node.span.end).is_some());
        if let Some(parent) = node.physical_parent {
            assert!(parent < node.ordinal);
            assert!(document.nodes[parent].span.contains(node.span));
        }
    }
    assert!(document.inline_records.iter().any(|record| {
        matches!(record.kind, InlineKind::Link(_))
            && record
                .destination
                .as_ref()
                .is_some_and(|text| text.value == "https://example.invalid/never-fetch")
    }));
}

#[test]
fn plain_long_text_is_raw_observation_without_decoded_copy() {
    let input = "x".repeat(HARD_LIMITS.metadata_bytes_each + 1);
    let cancel = AtomicBool::new(false);
    let document = parse_markdown(
        input.as_bytes(),
        HARD_LIMITS,
        CancellationToken::new(&cancel),
        deadline(),
    )
    .unwrap();
    assert_eq!(document.source, input);
    assert!(
        document
            .inline_records
            .iter()
            .any(|record| record.kind == InlineKind::Text && record.text.is_none())
    );
    assert_eq!(document.usage.metadata_bytes, 0);
    assert_eq!(
        document.usage.next_attempts,
        document.usage.yielded_events + 1
    );
}

#[test]
fn attempts_include_terminal_probe_and_refuse_before_extra_next() {
    let cancel = AtomicBool::new(false);
    let complete = parse_markdown(
        b"plain",
        HARD_LIMITS,
        CancellationToken::new(&cancel),
        deadline(),
    )
    .unwrap();
    let mut limits = HARD_LIMITS;
    limits.next_attempts = complete.usage.yielded_events;
    assert!(matches!(
        parse_markdown(
            b"plain",
            limits,
            CancellationToken::new(&cancel),
            deadline()
        ),
        Err(MarkdownError::AttemptLimit)
    ));
    limits.next_attempts += 1;
    let boundary = parse_markdown(
        b"plain",
        limits,
        CancellationToken::new(&cancel),
        deadline(),
    )
    .unwrap();
    assert_eq!(boundary.usage.next_attempts, limits.next_attempts);
}

#[test]
fn zero_budget_cancel_deadline_bom_and_bad_utf8_refuse() {
    let cancel = AtomicBool::new(false);
    let mut limits = HARD_LIMITS;
    limits.nodes = 0;
    assert!(matches!(
        parse_markdown(
            b"plain",
            limits,
            CancellationToken::new(&cancel),
            deadline()
        ),
        Err(MarkdownError::InvalidLimits)
    ));
    assert!(matches!(
        parse_markdown(
            b"plain",
            HARD_LIMITS,
            CancellationToken::new(&AtomicBool::new(true)),
            deadline()
        ),
        Err(MarkdownError::Cancelled)
    ));
    assert!(matches!(
        parse_markdown(
            b"plain",
            HARD_LIMITS,
            CancellationToken::new(&cancel),
            Instant::now()
        ),
        Err(MarkdownError::DeadlineExceeded)
    ));
    assert!(matches!(
        parse_markdown(
            b"\xef\xbb\xbfplain",
            HARD_LIMITS,
            CancellationToken::new(&cancel),
            deadline()
        ),
        Err(MarkdownError::BomNotSupported)
    ));
    assert!(matches!(
        parse_markdown(
            b"\xff",
            HARD_LIMITS,
            CancellationToken::new(&cancel),
            deadline()
        ),
        Err(MarkdownError::InvalidUtf8)
    ));
}

#[test]
fn headings_have_independent_physical_and_logical_parents() {
    let input = b"# outer\n\n> ## inner\n> text\n\n### following\n\ntext\n";
    let document = parse_markdown(
        input,
        HARD_LIMITS,
        CancellationToken::new(&AtomicBool::new(false)),
        deadline(),
    )
    .unwrap();
    let headings: Vec<_> = document
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, MarkdownKind::Heading { .. }))
        .collect();
    assert_eq!(headings.len(), 3);
    assert!(
        matches!(headings[1].kind, MarkdownKind::Heading { level: 2, parent_section: Some(parent) } if parent == headings[0].ordinal)
    );
    assert!(
        matches!(headings[2].kind, MarkdownKind::Heading { level: 3, parent_section: Some(parent) } if parent == headings[1].ordinal)
    );
    assert_ne!(headings[1].physical_parent, headings[1].heading_section);
    assert_eq!(headings[2].physical_parent, Some(0));
}

mod full_product {
    // AUTHORED_NOT_COMPILED / NOT_RUN.
    use super::super::model::{HARD_LIMITS, InlineKind, MarkdownKind};
    use crate::api::*;
    use crate::api::{
        MarkdownError as Error, MarkdownOperationLimits as Limits, MarkdownProfile as Md,
        MaterializationContext as Ctx, MaterializationError as BaseError,
        ValidatedMaterializationRequest as Req,
    };
    use crate::api::{materialize_markdown as make, verify_markdown as replay};
    use crate::markdown::identity::IdentityBudget;
    use crate::markdown::profile::MarkdownMeter;
    use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};
    use std::cell::Cell;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};
    const RAW: &str = "# café\r\n\r\n> enfant [lien](https://example.invalid/x)\r\n\r\n<script>never_execute()</script>\r\n";
    struct Port {
        bytes: Vec<u8>,
        digest: Blake3Digest32,
        calls: Cell<usize>,
        missing: bool,
    }
    impl RevisionReadPort for Port {
        fn read_exact(
            &self,
            s: &OpaqueId,
            r: NonZeroRevision,
            n: u64,
        ) -> Result<StoredRevisionBytes, BaseError> {
            self.calls.set(self.calls.get() + 1);
            assert_eq!(s.as_str(), "source:wrapper");
            assert_eq!(r.get(), 7);
            assert_eq!(n, RAW.len() as u64);
            if self.missing {
                return Err(BaseError::RevisionUnavailable);
            }
            Ok(StoredRevisionBytes::new(
                self.bytes.clone(),
                self.digest,
                OpaqueId::new("residency:wrapper").unwrap(),
            ))
        }
    }
    fn refuses<T>(got: Result<T, Error>, want: Error) {
        assert_eq!(got.err(), Some(want));
    }
    fn raw() -> MaterializerProfileDescriptor {
        let mut d = baseline_profile_descriptor("markdown-wrapper", 1);
        d.source_kinds = vec![SourceKind::Text];
        d.encodings = vec![SourceEncoding::Utf8];
        d.bom_policy = BomPolicy::RejectWhenPresent;
        d.newline_policy = NewlinePolicy::PreserveExact;
        d.loss_behavior = LossBehavior::RejectOnAnyLoss;
        d
    }
    fn case() -> (Req, Port, Ctx<'static>, Md) {
        let raw = validate_materializer_profile(&raw()).unwrap();
        let digest = Blake3Digest32::from_bytes(*blake3::hash(RAW.as_bytes()).as_bytes());
        let input = MaterializationRequest {
            source_id: OpaqueId::new("source:wrapper").unwrap(),
            revision: NonZeroRevision::new(7).unwrap(),
            residency: OpaqueId::new("residency:wrapper").unwrap(),
            content_digest: digest,
            byte_count: RAW.len() as u64,
            declared_kind: SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: raw.id(),
            operation_id: OpaqueId::new("operation:wrapper").unwrap(),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        };
        let r = validate_materialization_request(
            &input,
            &AcceptedProfiles::new(vec![raw.clone()]),
            &DEFAULT_MATERIALIZATION_BUDGET,
        )
        .unwrap();
        let f = Md::new(&raw, HARD_LIMITS).unwrap();
        let c = Ctx {
            profile: raw,
            budget: DEFAULT_MATERIALIZATION_BUDGET,
            cancel: CancellationToken::never(),
        };
        (
            r,
            Port {
                bytes: RAW.as_bytes().to_vec(),
                digest,
                calls: Cell::new(0),
                missing: false,
            },
            c,
            f,
        )
    }
    fn ready() -> (Req, Port, Ctx<'static>, Md, MarkdownProduct, Instant) {
        let (r, p, c, f) = case();
        let d = end();
        let x = make(&r, &p, &c, &f, caps(), d).unwrap();
        (r, p, c, f, x, d)
    }
    fn caps() -> Limits {
        Limits {
            parser: HARD_LIMITS,
            canonical_preimage_bytes: 2 * 1024 * 1024,
            total_canonical_bytes: 4 * 1024 * 1024,
        }
    }
    fn end() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }
    #[test]
    fn raw_roundtrip() {
        let (r, p, c, f, x, d) = ready();
        assert_eq!(x.canonical_text().as_bytes(), RAW.as_bytes());
        assert_eq!(
            x.raw_product().input_digest(),
            x.raw_product().canonical_digest()
        );
        assert!(x.raw_product().maps().loss_map().records().is_empty());
        assert_eq!(x.nodes()[0].span.end, RAW.len());
        assert!(
            x.nodes()
                .iter()
                .any(|n| n.kind == MarkdownKind::RawHtmlBlock)
        );
        assert!(x.inline_records().iter().any(|v| {
            matches!(v.kind, InlineKind::Link(_))
                && v.destination
                    .as_ref()
                    .is_some_and(|s| s.value == "https://example.invalid/x")
        }));
        let receipt = replay(&x, &r, &p, &c, &f, caps(), d).unwrap();
        assert_eq!(receipt.product_digest(), x.product_digest());
        assert_eq!(p.calls.get(), 2);
    }
    #[test]
    fn missing_child() {
        let (r, p, c, f, mut x, d) = ready();
        let i = x
            .inventory
            .nodes
            .iter()
            .position(|n| n.kind == MarkdownKind::Quote)
            .unwrap();
        let hash = x.product_digest();
        x.inventory.nodes.remove(i);
        assert_eq!(x.product_digest(), hash);
        refuses(
            replay(&x, &r, &p, &c, &f, caps(), d),
            Error::ProductMismatch,
        );
    }
    #[test]
    fn mapped_mutation() {
        let (r, p, c, f, mut x, d) = ready();
        let v = x
            .inventory
            .inline_records
            .iter_mut()
            .find(|v| v.destination.is_some())
            .unwrap();
        let span = v.span;
        let text = &mut v.destination.as_mut().unwrap().value;
        let len = text.len();
        text.replace_range(0..1, "x");
        assert_eq!(text.len(), len);
        assert_eq!(v.span, span);
        refuses(
            replay(&x, &r, &p, &c, &f, caps(), d),
            Error::ProductMismatch,
        );
    }
    #[test]
    fn revision_refusals() {
        let (r, mut p, c, f, x, d) = ready();
        p.bytes[0] = b'!'; // Same length/digest: hash actual bytes.
        refuses(
            replay(&x, &r, &p, &c, &f, caps(), d),
            Error::Materialization(BaseError::RevisionDigestMismatch),
        );
        p.missing = true;
        refuses(
            replay(&x, &r, &p, &c, &f, caps(), d),
            Error::Materialization(BaseError::RevisionUnavailable),
        );
    }
    #[test]
    fn strict_profiles() {
        for axis in 0..5 {
            let mut d = raw();
            match axis {
                0 => d.source_kinds.push(SourceKind::Code),
                1 => d.encodings.push(SourceEncoding::Utf16Le),
                2 => d.bom_policy = BomPolicy::StripAndRecord,
                3 => d.newline_policy = NewlinePolicy::NormalizeToLf,
                _ => d.loss_behavior = LossBehavior::RecordAndLowerAssurance,
            }
            let raw = validate_materializer_profile(&d).unwrap();
            refuses(Md::new(&raw, HARD_LIMITS), Error::ProfileMismatch);
        }
    }
    #[test]
    fn canonical_work_edges() {
        let (r, p, c, f, x, d) = ready();
        let mut low = caps();
        low.total_canonical_bytes = x.canonical_bytes_used();
        assert!(low.total_canonical_bytes > 1);
        assert_eq!(make(&r, &p, &c, &f, low, d).unwrap(), x);
        low.total_canonical_bytes -= 1;
        refuses(make(&r, &p, &c, &f, low, d), Error::CanonicalEncoding);
        low = caps();
        low.parser.work_steps = x.work_used();
        assert_eq!(make(&r, &p, &c, &f, low, d).unwrap(), x);
        low.parser.work_steps -= 1;
        refuses(make(&r, &p, &c, &f, low, d), Error::WorkLimit);
    }
    #[test]
    fn synthetic_row_and_replay() {
        let (r, p, c, f, x, d) = ready();
        let held = x.inventory.usage.owned_live + x.residency().as_str().len();
        // Synthetic payload bytes, not a full row.
        let row = x
            .inline_records()
            .iter()
            .find_map(|v| v.destination.as_ref())
            .unwrap()
            .value
            .len();
        let mut low = caps();
        low.parser.owned_bytes = held * 2 + row;
        let mut meter = MarkdownMeter::operation(low, c.cancel, d).unwrap();
        meter.hold_original_inventory(held).unwrap();
        meter.hold_retained(held).unwrap();
        meter.hold_transient(row).unwrap();
        assert_eq!(meter.remaining_storage(), 0);
        assert_eq!(meter.hold_transient(1), Err(Error::StorageLimit));
        meter.release_row();
        assert_eq!(meter.remaining_storage(), row);
        low.parser.owned_bytes = held + 1;
        let calls = p.calls.get();
        refuses(replay(&x, &r, &p, &c, &f, low, d), Error::StorageLimit);
        assert_eq!(p.calls.get(), calls + 1);
    }
    #[test]
    fn entry_checkpoints() {
        let (r, p, c, f) = case();
        let flag = AtomicBool::new(true);
        let cancelled = Ctx {
            profile: c.profile.clone(),
            budget: c.budget,
            cancel: CancellationToken::new(&flag),
        };
        let mut bad = caps();
        bad.parser.nodes = 0;
        refuses(make(&r, &p, &cancelled, &f, bad, end()), Error::Cancelled);
        refuses(
            make(&r, &p, &c, &f, bad, Instant::now()),
            Error::DeadlineExceeded,
        );
        assert_eq!(p.calls.get(), 0);
    }
}
