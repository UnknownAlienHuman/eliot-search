use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn service_output_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/service_output.rs");
    assert!(entry.contains("#[path = \"service_output/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub fn emit_indexed_source(",
        "pub fn write_line(",
        "pub fn json_string(",
        "StoreSearchResult",
        "SearchPage",
    ] {
        assert!(
            !entry.contains(forbidden),
            "service output implementation returned to entry: {forbidden}"
        );
    }

    let kernel = read(&root, "src/service_output/kernel.rs");
    for module in ["codec", "indexed", "page", "provider", "streaming"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    for export in [
        "MAX_RESPONSE_BYTES",
        "emit_indexed_source",
        "emit_search_page",
        "emit_handle_expansion",
        "emit_provider_status",
        "emit_streaming_search",
    ] {
        assert!(kernel.contains(export), "lost export {export}");
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn service_output_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        ("src/service_output/kernel/codec.rs", "pub fn write_line("),
        (
            "src/service_output/kernel/indexed.rs",
            "pub fn emit_indexed_source(",
        ),
        (
            "src/service_output/kernel/streaming.rs",
            "pub fn emit_streaming_search(",
        ),
        (
            "src/service_output/kernel/page.rs",
            "pub fn emit_search_page(",
        ),
        (
            "src/service_output/kernel/provider.rs",
            "pub fn emit_provider_status(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let codec = read(&root, "src/service_output/kernel/codec.rs");
    assert!(codec.contains("MAX_RESPONSE_BYTES: usize = 64 * 1024"));
    assert!(codec.contains("SERVICE_RESPONSE_TOO_LARGE"));
    assert!(codec.contains("diagnostics::sanitize_code"));
    assert!(codec.contains("\\u{:04x}"));
    assert!(!codec.contains("StoreSearchResult"));
    assert!(!codec.contains("SearchPage"));

    let indexed = read(&root, "src/service_output/kernel/indexed.rs");
    assert!(indexed.contains("source_indexed"));
    assert!(indexed.contains("durable_revision"));
    assert!(indexed.contains("diagnostic_internal_identifiers"));

    let streaming = read(&root, "src/service_output/kernel/streaming.rs");
    assert!(streaming.contains("corpus_search_started"));
    assert!(streaming.contains("source_gap"));
    assert!(streaming.contains("corpus_search_complete"));
    assert!(streaming.contains("emit_internal_match"));

    let page = read(&root, "src/service_output/kernel/page.rs");
    assert!(page.contains("SERVICE_HANDLE_PAGE_MISMATCH"));
    assert!(page.contains("search_page_started"));
    assert!(page.contains("search_page_complete"));
    assert!(page.contains("diagnostic_internal_identifiers\\\":false"));

    let provider = read(&root, "src/service_output/kernel/provider.rs");
    assert!(provider.contains("SERVICE_STATUS_TOO_LARGE"));
    assert!(provider.contains("provider_status"));
    assert!(provider.contains("source_handle_expanded"));
    assert!(provider.contains("sha256::hex"));
}

#[test]
fn service_output_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/service_output/kernel/tests.rs");
    assert!(tests.len() < 8_000, "output tests grew to {} bytes", tests.len());
    for case in [
        "error_frames_carry_closed_codes_only",
        "error_frames_redact_suffixes_paths_and_secrets",
        "response_ceiling_is_typed_and_bounded",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}
