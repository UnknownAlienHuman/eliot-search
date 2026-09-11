//! Parity tests for the T41 family-F slice (`xtask` vs Python).
//!
//! Covers only the ported pure helpers from `tools/coverage_graph_v2.py` and
//! `replace_once` from `tools/generate-coverage-graph-v2.py`. Vectors in
//! `fixtures/tooling/coverage-graph/vectors.json` were captured from `CPython`
//! 3.12 (`probe_cg*.py`) and asserted byte-exactly.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;
use xtask::coverage_graph::{
    arr, digest_text, heading_rows, module_refs_to_packages, quote_json, replace_once, slug, words,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/coverage-graph/vectors.json");
    let text = std::fs::read_to_string(&path).expect("parity vectors exist");
    serde_json::from_str(&text).expect("parity vectors parse")
}

fn hex_decode(hex: &str) -> Vec<u8> {
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let digit = |b: u8| {
        if b.is_ascii_digit() {
            b - b'0'
        } else if b.is_ascii_lowercase() {
            b - b'a' + 10
        } else {
            b - b'A' + 10
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        out.push(digit(bytes[i]) << 4 | digit(bytes[i + 1]));
        i += 2;
    }
    out
}

// slug: hello-world + empty/dash fallback.
#[test]
fn slug_basic_and_fallback_pass() {
    let v = vectors();
    assert_eq!(slug("Hello, World!"), v["slug_hello"].as_str().unwrap());
    assert_eq!(slug(""), v["slug_empty"].as_str().unwrap());
    assert_eq!(slug("---"), v["slug_dashes"].as_str().unwrap());
}

// slug: truncation, spaces, unicode.
#[test]
fn slug_truncation_spaces_unicode_pass() {
    let v = vectors();
    let long = slug(&"A".repeat(200));
    assert_eq!(
        long.len(),
        usize::try_from(v["slug_long_len"].as_u64().unwrap()).unwrap()
    );
    assert!(long.chars().all(|c| c == 'a'));
    assert_eq!(slug("  A B  "), v["slug_spaces"].as_str().unwrap());
    assert_eq!(
        slug("Caf\u{e9}_M\u{fc}nchen!"),
        v["slug_uni"].as_str().unwrap()
    );
}

// slug: never empty, bounded, charset-only (negative shape).
#[test]
fn slug_output_shape_rejected_when_violated() {
    for input in ["", "---", "  !!!  ", "\u{00e9}\u{00fc}"] {
        let s = slug(input);
        assert!(!s.is_empty());
        assert!(s.len() <= 96);
    }
    let s = slug("a b@c");
    assert!(
        s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    );
    assert_ne!(slug("a"), slug("b"));
}

// digest: abc + empty vectors.
#[test]
fn digest_vectors_pass() {
    let v = vectors();
    assert_eq!(digest_text("abc"), v["digest_abc"].as_str().unwrap());
    assert_eq!(digest_text(""), v["digest_empty"].as_str().unwrap());
}

// digest: 64 lower-hex, order-sensitive (negative).
#[test]
fn digest_shape_and_sensitivity_rejected_when_violated() {
    let d = digest_text("abc");
    assert_eq!(d.len(), 64);
    assert!(
        d.bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    );
    assert_ne!(digest_text("abc"), digest_text("abd"));
    assert_ne!(digest_text(""), digest_text(" "));
}

// q: empty/nl/c0/emoji/cafe byte-exact via hex vectors.
#[test]
fn quote_vectors_pass() {
    let v = vectors();
    for key in [
        "q_empty_hex",
        "q_nl_hex",
        "q_c0_hex",
        "q_emoji_hex",
        "q_cafe_hex",
    ] {
        let _ = v[key].as_str().expect("hex vector");
    }
    assert_eq!(
        quote_json("").as_bytes(),
        hex_decode(v["q_empty_hex"].as_str().unwrap())
    );
    assert_eq!(
        quote_json("a\nb\t\"\\").as_bytes(),
        hex_decode(v["q_nl_hex"].as_str().unwrap())
    );
    assert_eq!(
        quote_json("\u{0}\u{1f}").as_bytes(),
        hex_decode(v["q_c0_hex"].as_str().unwrap())
    );
    assert_eq!(
        quote_json("\u{1f600}").as_bytes(),
        hex_decode(v["q_emoji_hex"].as_str().unwrap())
    );
    assert_eq!(
        quote_json("caf\u{e9}").as_bytes(),
        hex_decode(v["q_cafe_hex"].as_str().unwrap())
    );
}

// q shape: quoted, control-free (negative).
#[test]
fn quote_shape_rejected_when_violated() {
    let q = quote_json("a\"b\\c");
    assert!(q.starts_with('"') && q.ends_with('"'));
    assert!(q.contains("\\\"") && q.contains("\\\\"));
    assert_ne!(quote_json("a"), quote_json("b"));
}

// arr: empty/one/two vectors, order preserved.
#[test]
fn arr_vectors_and_order_pass() {
    let v = vectors();
    assert_eq!(arr(&[]), v["arr_empty"].as_str().unwrap());
    assert_eq!(arr(&["x"]), v["arr_one"].as_str().unwrap());
    assert_eq!(arr(&["b", "a"]), v["arr_two"].as_str().unwrap());
    assert_ne!(arr(&["b", "a"]), arr(&["a", "b"]));
}

// words: normalize + bus/running/tested stems.
#[test]
fn words_normalize_and_stems_pass() {
    let v = vectors();
    let got: Vec<String> = words("normalizeInput-value").into_iter().collect();
    let want: Vec<String> = v["words_normalize"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(got, want);
    let got2: Vec<String> = words("buses running tested").into_iter().collect();
    let want2: Vec<String> = v["words_buses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(got2, want2);
}

// words: cities->city, camel parseHTTPResponse.
#[test]
fn words_cities_and_camel_pass() {
    let v = vectors();
    let got: BTreeSet<String> = words("cities");
    let want: BTreeSet<String> = v["words_cities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(got, want);
    let got2: Vec<String> = words("parseHTTPResponse").into_iter().collect();
    let want2: Vec<String> = v["words_camel"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(got2, want2);
}

// words: empty + lowercase ascii-only (negative).
#[test]
fn words_empty_and_shape_rejected_when_violated() {
    assert!(words("").is_empty());
    for token in words("Hello_World-TEST") {
        assert!(
            token
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        );
    }
}

// headings: two-row basic case from CPython probe.
#[test]
fn heading_basic_rows_pass() {
    let rows = heading_rows("# Title\n## Sub `code` **bold**\n##### too deep\n#NoSpace\n");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].line, 1);
    assert_eq!(rows[0].level, 1);
    assert_eq!(rows[0].raw, "Title");
    assert_eq!(rows[0].title, "Title");
    assert_eq!(rows[1].line, 2);
    assert_eq!(rows[1].level, 2);
    assert_eq!(rows[1].raw, "Sub `code` **bold**");
    assert_eq!(rows[1].title, "Sub code bold");
}

// headings: L4 kept, L5/NoSpace/indent dropped.
#[test]
fn heading_levels_and_indent_pass() {
    let rows = heading_rows("#  spaced   \n#### L4 ok\n##### L5 no\n#NoSpace\n   # indent\n");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].raw, "spaced");
    assert_eq!(rows[0].level, 1);
    assert_eq!(rows[1].raw, "L4 ok");
    assert_eq!(rows[1].level, 4);
}

// headings: empty / no-heading text yields none (negative).
#[test]
fn heading_empty_yields_none() {
    assert!(heading_rows("").is_empty());
    assert!(heading_rows("plain\n- list\n").is_empty());
    assert!(heading_rows("##### deep only\n#NoSpace\n").is_empty());
}

// replace_once: single occurrence ok.
#[test]
fn replace_once_ok_pass() {
    assert_eq!(replace_once("aXb", "X", "Y", "lbl").unwrap(), "aYb");
}

// replace_once: zero/two occurrences rejected with exact messages.
#[test]
fn replace_once_counts_rejected() {
    let err0 = replace_once("aaa", "X", "Y", "lbl")
        .unwrap_err()
        .to_string();
    assert_eq!(err0, "lbl: expected one occurrence, found 0");
    let err2 = replace_once("aXbXc", "X", "Y", "lbl")
        .unwrap_err()
        .to_string();
    assert_eq!(err2, "lbl: expected one occurrence, found 2");
}

// module refs: sorted unique packages.
#[test]
fn module_refs_sorted_unique_pass() {
    assert_eq!(
        module_refs_to_packages(&["a:x", "b:y", "a:z"]),
        vec!["a".to_owned(), "b".to_owned()]
    );
}

// module refs: empty + colon-less (negative shape).
#[test]
fn module_refs_edge_pass() {
    assert!(module_refs_to_packages(&[]).is_empty());
    assert_eq!(module_refs_to_packages(&["solo"]), vec!["solo".to_owned()]);
    assert_eq!(
        module_refs_to_packages(&["b:y", "a:x"]),
        vec!["a".to_owned(), "b".to_owned()]
    );
}
