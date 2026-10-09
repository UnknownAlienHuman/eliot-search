use serde_json::{Value, json};
use syn::visit::Visit;

use super::{Key, Site, Sites, allowlist, detector::Detector, syntax};

fn detect(source: &str) -> Detector {
    let tokens = syntax::tokens(source).expect("bounded tokens");
    let file = syn::parse2::<syn::File>(tokens).expect("fixture AST");
    let mut detector = Detector::new(false);
    detector.visit_file(&file);
    assert!(detector.error.is_none());
    detector
}

fn has(detector: &Detector, signal: &str, test: bool) -> bool {
    detector
        .findings
        .keys()
        .any(|(_, found, context)| found == signal && *context == test)
}

#[test]
fn production_beside_test_is_never_suppressed() {
    let d = detect(
        "fn production() { Blake3Digest32::from_bytes([7;32]); } #[cfg(test)] mod tests { fn fixture() { Blake3Digest32::from_bytes([1;32]); } }",
    );
    assert!(has(&d, "raw-digest-construction", false));
    assert!(has(&d, "raw-digest-construction", true));
}

#[test]
fn question_mark_and_name_do_not_prove_algorithm_or_decode() {
    let d = detect("fn fake() { let bytes = hash(input)?; Blake3Digest32::from_bytes(bytes); }");
    assert!(has(&d, "raw-digest-construction", false));
    assert!(has(&d, "named-compute-review", false));
    assert!(!has(&d, "crypto-call-review", false));
}

#[test]
fn real_crypto_and_restore_are_distinct_candidates() {
    let d = detect(
        "fn compute() { blake3::hash(b\"abc\"); } fn restore() { Blake3Digest32::from_stored_bytes(bytes); }",
    );
    assert!(has(&d, "crypto-call-review", false));
    assert!(
        d.findings
            .keys()
            .any(|(symbol, signal, _)| symbol == "restore" && signal == "raw-digest-construction")
    );
}

#[test]
fn copied_sha_mixer_and_prefix_are_reviewed() {
    let d = detect(
        "const K:u32=0x428a2f98; fn copied() { let a=n.rotate_right(7); let b=a.wrapping_mul(3); let id=&b[..16]; }",
    );
    for signal in [
        "algorithm-constant-review",
        "mixer-review",
        "digest-prefix-review",
    ] {
        assert!(has(&d, signal, false));
    }
}

#[test]
fn macros_and_renamed_crypto_import_need_resolution() {
    let d = detect("use sha2::Sha256 as Innocent; mint!(Blake3Digest32::from_bytes([1;32]));");
    assert!(has(&d, "crypto-import-review", false));
    assert!(has(&d, "digest-macro-review", false));
}

#[test]
fn trait_writers_raw_conversions_and_split_prefixes_are_reviewed() {
    let d = detect(
        "trait Encoder { fn canonical_bytes(&self) -> &[u8]; } impl From<[u8;32]> for Sha256Digest32 { fn from(v:[u8;32])->Self { Self(v) } } fn id(v:&[u8]) { let (p,_) = v.split_at(4); }",
    );
    for signal in [
        "canonical-writer-review",
        "digest-conversion-review",
        "short-prefix-review",
    ] {
        assert!(has(&d, signal, false));
    }
}

fn fixture_sites() -> (Sites, Value) {
    let key = Key {
        path: "x.rs".into(),
        symbol: "restore".into(),
        signal: "raw-digest-construction".into(),
    };
    let site = Site {
        key: key.clone(),
        count: 1,
        test: false,
        source_sha256: "a".repeat(64),
    };
    let mut entry = site.json();
    entry["classification"] = json!("wire-store-decode");
    entry["reason"] = json!("exact stored field decode; does not assert compute");
    entry["owner_issue"] = json!(237);
    entry["phase"] = json!("accepted");
    (
        Sites::from([(key, site)]),
        json!({"version":1,"owners":{"237":{"state":"OPEN"}},"sites":[entry]}),
    )
}

#[test]
fn exact_decode_fixture_passes_but_new_site_fails() {
    let (mut sites, ledger) = fixture_sites();
    assert!(allowlist::validate_value(&ledger, &sites).is_ok());
    let mut site = sites.values().next().expect("site").clone();
    site.key.symbol = "fake".into();
    sites.insert(site.key.clone(), site);
    assert!(allowlist::validate_value(&ledger, &sites).is_err());
}

#[test]
fn changed_source_unresolved_and_production_test_label_fail() {
    let (sites, mut ledger) = fixture_sites();
    for (field, value) in [
        ("source_sha256", json!("b".repeat(64))),
        ("classification", json!("COMPILER_REQUIRED")),
        ("classification", json!("test-vector")),
        ("count", json!(2)),
    ] {
        let previous = ledger["sites"][0][field].clone();
        ledger["sites"][0][field] = value;
        assert!(allowlist::validate_value(&ledger, &sites).is_err());
        ledger["sites"][0][field] = previous;
    }
}

#[test]
fn bounded_token_preflight_rejects_deep_groups_and_chains() {
    assert!(syntax::tokens(&format!("{}0{}", "(".repeat(66), ")".repeat(66))).is_err());
    assert!(
        syntax::tokens(&format!(
            "fn f() {{ let x = {}; }}",
            vec!["1"; 140].join("+")
        ))
        .is_err()
    );
}
