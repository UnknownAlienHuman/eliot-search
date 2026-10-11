//! AUTHORED_NOT_COMPILED/NOT_RUN. Execution belongs to #215/#216.
use super::identity::{inventory_identity, product_identity};
use super::meter::Meter;
use super::model::{JatsElementKind as K, JatsError as E};
use super::pipeline::{JatsProduct, materialize_jats as make, verify_jats as replay};
use super::profile::{JatsOperationLimits, JatsProfile, MAX_JATS_LIMITS};
use crate::api::*;
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const ARTICLE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><article dtd-version=\"1.4\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:xml=\"http://www.w3.org/XML/1998/namespace\"><front><journal-meta><journal-id journal-id-type=\"publisher-id\">fixture</journal-id><issn publication-format=\"electronic\">1234-5678</issn></journal-meta><article-meta><title-group><article-title>Café <italic>title</italic></article-title><subtitle>sub</subtitle></title-group><abstract><p>résumé</p></abstract></article-meta></front><body><sec id=\"s1\"><title>Section</title><p xml:lang=\"fr\">A\r\nB\rC &amp;&lt;&gt;&apos;&quot;&#13;&#xE9;<bold>B</bold><sup>1</sup><sub>2</sub><monospace>m</monospace><underline>u</underline><named-content content-type=\"term\">n</named-content><xref rid=\"r1\" ref-type=\"bibr\">1</xref><ext-link xlink:href=\"https://example.invalid/never-fetch\">link</ext-link><![CDATA[raw\r\ntext]]><!--inert--></p><list list-type=\"bullet\"><list-item><p>item</p></list-item></list><boxed-text><p>box</p></boxed-text><disp-quote><p>quote</p></disp-quote><code>code</code><preformat>pre</preformat><fig id=\"f1\"><label>1</label><caption><p>figure</p></caption></fig><table-wrap><caption><p>table</p></caption><table><thead><tr><th>head</th></tr></thead><tbody><tr><td colspan=\"1\">cell</td><td/></tr></tbody></table></table-wrap></sec></body><back><ack><p>thanks</p></ack><fn-group><fn id=\"fn1\"><p>note</p></fn></fn-group><ref-list><ref id=\"r1\"><mixed-citation>Author, <article-title>title</article-title>.</mixed-citation></ref><ref><element-citation><article-title>title</article-title></element-citation></ref></ref-list></back></article>";
const FRONT: &str = "<front><journal-meta><journal-id>fixture</journal-id><issn>1234-5678</issn></journal-meta><article-meta><title-group><article-title>title</article-title></title-group></article-meta></front>";
fn article(content: &str) -> String {
    format!("<article>{FRONT}<body>{content}</body></article>")
}
fn caps() -> JatsOperationLimits {
    JatsOperationLimits {
        parser: MAX_JATS_LIMITS,
        canonical_preimage_bytes: 2 * 1024 * 1024,
        total_canonical_bytes: 4 * 1024 * 1024,
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}
struct Port {
    bytes: Vec<u8>,
    digest: Blake3Digest32,
    calls: Cell<usize>,
    cancel_at_return: Option<&'static AtomicBool>,
    expire_at_return: Option<Instant>,
}
impl RevisionReadPort for Port {
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        assert_eq!(source.as_str(), "source:jats-fixture");
        assert_eq!(revision.get(), 7);
        assert_eq!(count, u64::try_from(self.bytes.len()).unwrap());
        self.calls.set(self.calls.get() + 1);
        if let Some(flag) = self.cancel_at_return {
            flag.store(true, Ordering::Release);
        }
        if let Some(end) = self.expire_at_return {
            std::thread::sleep(end.saturating_duration_since(Instant::now()));
        }
        Ok(StoredRevisionBytes::new(
            self.bytes.clone(),
            self.digest,
            OpaqueId::new("residency:jats-fixture").unwrap(),
        ))
    }
}
fn case(
    raw_xml: &str,
) -> (
    ValidatedMaterializationRequest,
    Port,
    MaterializationContext<'static>,
    JatsProfile,
) {
    let mut descriptor = baseline_profile_descriptor("jats-fixture", 1);
    descriptor.source_kinds = vec![SourceKind::Text];
    descriptor.encodings = vec![SourceEncoding::Utf8];
    descriptor.bom_policy = BomPolicy::RejectWhenPresent;
    descriptor.newline_policy = NewlinePolicy::PreserveExact;
    descriptor.loss_behavior = LossBehavior::RejectOnAnyLoss;
    let raw = validate_materializer_profile(&descriptor).unwrap();
    let digest = Blake3Digest32::from_bytes(*blake3::hash(raw_xml.as_bytes()).as_bytes());
    let input = MaterializationRequest {
        source_id: OpaqueId::new("source:jats-fixture").unwrap(),
        revision: NonZeroRevision::new(7).unwrap(),
        residency: OpaqueId::new("residency:jats-fixture").unwrap(),
        content_digest: digest,
        byte_count: u64::try_from(raw_xml.len()).unwrap(),
        declared_kind: SourceKind::Text,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: raw.id(),
        operation_id: OpaqueId::new("operation:jats-fixture").unwrap(),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    let request = validate_materialization_request(
        &input,
        &AcceptedProfiles::new(vec![raw.clone()]),
        &DEFAULT_MATERIALIZATION_BUDGET,
    )
    .unwrap();
    let profile = JatsProfile::new(&raw, MAX_JATS_LIMITS).unwrap();
    let context = MaterializationContext {
        profile: raw,
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::never(),
    };
    (
        request,
        Port {
            bytes: raw_xml.as_bytes().to_vec(),
            digest,
            calls: Cell::new(0),
            cancel_at_return: None,
            expire_at_return: None,
        },
        context,
        profile,
    )
}
#[derive(Clone, Copy)]
enum Action {
    Cancel(&'static AtomicBool),
    Expire,
}
thread_local! {
    static ARM: Cell<Option<(bool, Action)>> = const { Cell::new(None) };
    static HIT: Cell<usize> = const { Cell::new(0) };
}
pub(super) fn terminal(meter: &Meter<'_>, verifier: bool) {
    ARM.with(|arm| {
        if let Some((target, action)) = arm.take() {
            if target != verifier {
                arm.set(Some((target, action)));
                return;
            }
            HIT.with(|hit| hit.set(hit.get() + 1));
            match action {
                Action::Cancel(flag) => flag.store(true, Ordering::Release),
                Action::Expire => {
                    let end = meter.operation.unwrap().1;
                    std::thread::sleep(end.saturating_duration_since(Instant::now()));
                }
            }
        }
    });
}
fn rehash(
    product: &mut JatsProduct,
    request: &ValidatedMaterializationRequest,
    profile: &JatsProfile,
) {
    let mut meter = Meter {
        limits: caps(),
        operation: None,
        work: 0,
        owned: 0,
        canonical: 0,
    };
    product.inventory_digest = inventory_identity(&product.inventory, &mut meter).unwrap();
    product.digest = product_identity(
        &product.raw,
        request,
        profile,
        product.inventory_digest,
        &mut meter,
    )
    .unwrap();
    product.canonical_bytes = meter.canonical;
}
#[test]
fn full_subset_original_ranges_and_owning_replay() {
    let (r, p, c, f) = case(ARTICLE);
    let d = deadline();
    let product = make(&r, &p, &c, &f, caps(), d).unwrap();
    assert_eq!(product.canonical_text().as_bytes(), ARTICLE.as_bytes());
    assert_eq!(
        product.raw_product().input_digest(),
        product.raw_product().canonical_digest()
    );
    assert!(product.raw_product().maps().loss_map().records().is_empty());
    let expected = [
        K::Article,
        K::Front,
        K::JournalMeta,
        K::JournalId,
        K::Issn,
        K::ArticleMeta,
        K::TitleGroup,
        K::ArticleTitle,
        K::Italic,
        K::Subtitle,
        K::Abstract,
    ];
    assert_eq!(
        product
            .elements()
            .iter()
            .take(expected.len())
            .map(|v| v.kind)
            .collect::<Vec<_>>(),
        expected
    );
    for (i, value) in product.elements().iter().enumerate() {
        assert!(ARTICLE.get(value.span.start..value.span.end).is_some());
        if let Some(parent) = value.parent {
            assert!(parent < i);
            assert!(product.elements()[parent].span.start <= value.span.start);
            assert!(value.span.end <= product.elements()[parent].span.end);
        }
    }
    for kind in [
        K::ListItem,
        K::BoxedText,
        K::Quote,
        K::Code,
        K::Preformat,
        K::Figure,
        K::TableWrap,
        K::TableHead,
        K::TableBody,
        K::HeaderCell,
        K::Cell,
        K::Acknowledgment,
        K::FootnoteGroup,
        K::Reference,
        K::MixedCitation,
        K::ElementCitation,
    ] {
        assert!(product.elements().iter().any(|v| v.kind == kind));
    }
    assert_eq!(
        replay(&product, &r, &p, &c, &f, caps(), d)
            .unwrap()
            .product_digest(),
        product.digest
    );
    assert_eq!(p.calls.get(), 2);
}
#[test]
fn mapped_normalization_and_numeric_cr_are_not_raw_subanchors() {
    let xml = article(
        "<p xml:lang=\"a&#13;b&#xE9;\">A\r\nB\rC&amp;&lt;&gt;&apos;&quot;&#13;&#xE9;<![CDATA[D\r\nE]]></p>",
    );
    let (r, p, c, f) = case(&xml);
    let product = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
    let paragraph = product
        .elements()
        .iter()
        .position(|v| v.kind == K::Paragraph)
        .unwrap();
    let text = product
        .texts()
        .iter()
        .filter(|v| v.owner == paragraph)
        .map(|v| v.value.as_str())
        .collect::<String>();
    assert_eq!(text, "A\nB\nC&<>'\"\réD\nE");
    let attr = product
        .attributes()
        .iter()
        .find(|v| v.kind == super::model::JatsAttributeKind::Lang)
        .unwrap();
    assert_eq!(attr.value, "a\rbé");
    assert!(xml[attr.span.start..attr.span.end].starts_with("<p "));
    let cr = product.texts().iter().find(|v| v.value == "\r").unwrap();
    assert_eq!(&xml[cr.span.start..cr.span.end], "&#13;");
}
#[test]
fn closed_xml_syntax_and_namespace_refusals() {
    for xml in [
        "<article/>",
        "<article><front/></article>",
        "<article><front></article>",
        "<article><front>",
    ] {
        let (r, p, c, f) = case(xml);
        assert!(make(&r, &p, &c, &f, caps(), deadline()).is_err(), "{xml}");
    }
    let valid = article("<p/>");
    let (r, p, c, f) = case(&valid);
    assert!(make(&r, &p, &c, &f, caps(), deadline()).is_ok());
    for prefix in [
        "<!DOCTYPE article SYSTEM 'https://example.invalid/x'>",
        "<?worker x?>",
        "<?xml version='1.1'?>",
        "<?xml version='1.0' version='1.0'?>",
        "<?xml version='1.0' encoding='UTF-16'?>",
        "<?xml version='1.0' standalone='yes' encoding='UTF-8'?>",
        "<?xml version='1.0'encoding='UTF-8'?>",
    ] {
        let xml = format!("{prefix}{valid}");
        let (r, p, c, f) = case(&xml);
        assert!(
            make(&r, &p, &c, &f, caps(), deadline()).is_err(),
            "{prefix}"
        );
    }
    for content in [
        "<unknown/>",
        "<p unknown='x'/>",
        "<p id='a' id='b'/>",
        "<p id='a'xml:lang='en'/>",
        "<p>&custom;</p>",
        "<p>&#1;</p>",
        "<p id='&#xFFFE;'/>",
        "<p>\u{1}</p>",
        "<p>]]></p>",
        "<p id='a<b'/>",
        "<p><!--a--b--></p>",
        "<p xmlns:xlink='http://www.w3.org/1999/xlink'/>",
        "<p xmlns=''/>",
        "<p xmlns='urn:unknown'/>",
        "<x:p/>",
        "<p xlink:href='x'/>",
    ] {
        let xml = article(content);
        let (r, p, c, f) = case(&xml);
        assert!(
            make(&r, &p, &c, &f, caps(), deadline()).is_err(),
            "{content}"
        );
    }
}
#[test]
fn normalized_attribute_lower_ceiling_is_effective_in_producer_and_replay() {
    let xml = "<article><front><journal-meta><journal-id/><issn/></journal-meta><article-meta><title-group><article-title/></title-group></article-meta></front><body><p id='ab'/></body></article>";
    let (r, p, c, f) = case(xml);
    let product = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
    let mut limits = caps();
    limits.parser.mapped_bytes_each = 2;
    assert!(make(&r, &p, &c, &f, limits, deadline()).is_ok());
    assert!(replay(&product, &r, &p, &c, &f, limits, deadline()).is_ok());
    limits.parser.mapped_bytes_each = 1;
    let before = p.calls.get();
    assert_eq!(
        make(&r, &p, &c, &f, limits, deadline()).err(),
        Some(E::LimitExceeded)
    );
    assert_eq!(p.calls.get(), before + 1);
    let before = p.calls.get();
    assert_eq!(
        replay(&product, &r, &p, &c, &f, limits, deadline()).err(),
        Some(E::LimitExceeded)
    );
    assert_eq!(p.calls.get(), before + 1);
}
#[test]
fn preflight_and_live_port_checkpoints_preserve_original_inputs() {
    let (r, mut p, mut c, f) = case(ARTICLE);
    let mut limits = caps();
    limits.parser.input_bytes = ARTICLE.len() - 1;
    assert_eq!(
        make(&r, &p, &c, &f, limits, deadline()).err(),
        Some(E::LimitExceeded)
    );
    assert_eq!(p.calls.get(), 0);
    limits = caps();
    limits.parser.work_steps = 1;
    assert_eq!(
        make(&r, &p, &c, &f, limits, deadline()).err(),
        Some(E::LimitExceeded)
    );
    assert_eq!(p.calls.get(), 0);
    let flag = Box::leak(Box::new(AtomicBool::new(false)));
    c.cancel = CancellationToken::new(flag);
    p.cancel_at_return = Some(flag);
    assert_eq!(
        make(&r, &p, &c, &f, caps(), deadline()).err(),
        Some(E::Cancelled)
    );
    assert_eq!(p.calls.get(), 1);
    flag.store(false, Ordering::Release);
    p.cancel_at_return = None;
    let end = Instant::now() + Duration::from_secs(1);
    p.expire_at_return = Some(end);
    let before = p.calls.get();
    assert_eq!(
        make(&r, &p, &c, &f, caps(), end).err(),
        Some(E::DeadlineExceeded)
    );
    assert_eq!(p.calls.get(), before + 1);
}
#[test]
fn causal_producer_and_verifier_terminal_checkpoints() {
    for verifier in [false, true] {
        for expire in [false, true] {
            let (r, p, mut c, f) = case(ARTICLE);
            let flag = Box::leak(Box::new(AtomicBool::new(false)));
            c.cancel = CancellationToken::new(flag);
            let original = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
            let end = Instant::now() + Duration::from_secs(1);
            let action = if expire {
                Action::Expire
            } else {
                Action::Cancel(flag)
            };
            HIT.with(|v| v.set(0));
            ARM.with(|v| v.set(Some((verifier, action))));
            let before = p.calls.get();
            let refusal = if verifier {
                replay(&original, &r, &p, &c, &f, caps(), end).err()
            } else {
                make(&r, &p, &c, &f, caps(), end).err()
            };
            assert_eq!(
                refusal,
                Some(if expire {
                    E::DeadlineExceeded
                } else {
                    E::Cancelled
                })
            );
            HIT.with(|v| assert_eq!(v.get(), 1));
            assert_eq!(p.calls.get(), before + 1);
        }
    }
}
#[test]
fn complete_rehashed_inventory_mutation_matrix() {
    for mode in 0..10 {
        let (r, p, c, f) = case(ARTICLE);
        let mut product = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
        match mode {
            0 => {
                product.inventory.texts.pop();
            }
            1 => {
                product.inventory.attributes.pop();
            }
            2 => {
                product.inventory.comments.clear();
            }
            3 => {
                product.inventory.texts.swap(0, 1);
            }
            4 => {
                product.inventory.elements[8].kind = K::Bold;
            }
            5 => {
                product.inventory.elements[8].span.end -= 1;
            }
            6 => {
                product.inventory.attributes[0].value.push('x');
            }
            7 => product.inventory.declaration = None,
            8 => product.inventory.events -= 1,
            _ => product.inventory.mapped -= 1,
        }
        rehash(&mut product, &r, &f);
        assert_eq!(
            replay(&product, &r, &p, &c, &f, caps(), deadline()).err(),
            Some(E::ProductMismatch)
        );
    }
}
#[test]
fn chunked_inventory_exceeds_shared_single_array_cap_without_omission() {
    let xml = article(&"<p id='x'>x&amp;y</p>".repeat(1000));
    let (r, p, c, f) = case(&xml);
    let product = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
    assert!(product.inventory.count().unwrap() > 4096);
    assert_eq!(
        product
            .elements()
            .iter()
            .filter(|v| v.kind == K::Paragraph)
            .count(),
        1000
    );
    assert_eq!(product.attributes().len(), 1000);
    assert_eq!(
        product
            .texts()
            .iter()
            .filter(|v| product.elements()[v.owner].kind == K::Paragraph)
            .count(),
        3000
    );
    assert!(replay(&product, &r, &p, &c, &f, caps(), deadline()).is_ok());
}
#[test]
fn exact_complete_canonical_limit_and_held_replay_refusal() {
    let (r, p, c, f) = case(ARTICLE);
    let product = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
    let mut limit = caps();
    limit.total_canonical_bytes = product.canonical_bytes_used();
    assert!(make(&r, &p, &c, &f, limit, deadline()).is_ok());
    assert!(replay(&product, &r, &p, &c, &f, limit, deadline()).is_ok());
    limit.total_canonical_bytes -= 1;
    assert!(make(&r, &p, &c, &f, limit, deadline()).is_err());
    assert!(replay(&product, &r, &p, &c, &f, limit, deadline()).is_err());
    let mut meter = Meter {
        limits: caps(),
        operation: None,
        work: 0,
        owned: 0,
        canonical: 0,
    };
    inventory_identity(&product.inventory, &mut meter).unwrap();
    let inventory_bytes = meter.canonical;
    let product_bytes = product.canonical_bytes_used() - inventory_bytes;
    limit = caps();
    limit.canonical_preimage_bytes = inventory_bytes.max(product_bytes);
    assert!(make(&r, &p, &c, &f, limit, deadline()).is_ok());
    assert!(replay(&product, &r, &p, &c, &f, limit, deadline()).is_ok());
    limit.canonical_preimage_bytes -= 1;
    assert!(make(&r, &p, &c, &f, limit, deadline()).is_err());
    assert!(replay(&product, &r, &p, &c, &f, limit, deadline()).is_err());
    limit = caps();
    limit.parser.owned_bytes = 1;
    assert_eq!(
        replay(&product, &r, &p, &c, &f, limit, deadline()).err(),
        Some(E::LimitExceeded)
    );
}
#[test]
fn privacy_and_profile_narrowing() {
    let (r, p, c, f) = case(ARTICLE);
    let product = make(&r, &p, &c, &f, caps(), deadline()).unwrap();
    assert!(!format!("{product:?}").contains("never-fetch"));
    assert!(!format!("{:?}", product.texts()).contains("résumé"));
    assert!(!format!("{:?}", product.attributes()).contains("never-fetch"));
    let mut limits = MAX_JATS_LIMITS;
    limits.depth -= 1;
    let different = JatsProfile::new(&c.profile, limits).unwrap();
    let mut operation = caps();
    operation.parser = limits;
    assert_ne!(different.id(), f.id());
    assert_eq!(
        replay(&product, &r, &p, &c, &different, operation, deadline()).err(),
        Some(E::ProductMismatch)
    );
}
