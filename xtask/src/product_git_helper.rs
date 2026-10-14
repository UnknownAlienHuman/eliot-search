//! Product-owned repository path grammar and read-limit helpers.
//!
//! The conservative path grammar and byte ceiling used by advisory Git-tree readers are
//! product concerns of the Search repository, not controller-planner concerns. This module holds
//! the single product-owned copy, so readers no longer depend on retired ticket-issuance
//! planning. Behaviour is byte-identical to the historical planner helper.

/// Maximum committed blob size that reader tooling will read: 4 MiB.
pub const HELPER_FILE_BYTE_CEILING: u64 = 4 * 1024 * 1024;

/// Repository-relative safe path check.
///
/// Mirrors the conservative repository grammar plus `PurePosixPath` semantics: no absolute,
/// empty or parent segments after dropping single-dot segments. A single-dot segment is
/// therefore still an accepted (redundant) interior segment.
#[must_use]
pub fn helper_safe_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let grammar = value.split('/').all(|part| {
        !part.is_empty()
            && part.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
    });
    grammar
        && value
            .split('/')
            .filter(|part| *part != ".")
            .all(|part| part != "..")
}

/// Path equality or strict `prefix/` containment.
#[must_use]
pub fn helper_under(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_path_grammar_is_conservative() {
        assert!(helper_safe_path("Cargo.toml"));
        assert!(helper_safe_path("crates/search-contracts/src/lib.rs"));
        assert!(helper_safe_path("a/./b"));
        assert!(!helper_safe_path(""));
        assert!(!helper_safe_path("/etc/passwd"));
        assert!(!helper_safe_path("a//b"));
        assert!(!helper_safe_path("a/../b"));
        assert!(!helper_safe_path(".."));
        assert!(!helper_safe_path("crates/search contracts/src/lib.rs"));
    }

    #[test]
    fn under_is_equality_or_directory_containment() {
        assert!(helper_under("docs", "docs"));
        assert!(helper_under("docs/adr/0005.md", "docs"));
        assert!(!helper_under("docs-archive", "docs"));
        assert!(!helper_under("do", "docs"));
    }

    #[test]
    fn planner_aliases_delegate_to_the_same_grammar() {
        for case in [
            "Cargo.toml",
            "crates/search-contracts/src/lib.rs",
            "",
            "/etc/passwd",
            "a//b",
            "a/../b",
            "a/./b",
            "..",
        ] {
            assert_eq!(
                helper_safe_path(case),
                crate::ticket_planner::safe_path(case),
                "divergent safe_path for {case:?}"
            );
        }
        for (path, prefix) in [("docs", "docs"), ("docs/a.md", "docs"), ("docs-x", "docs")] {
            assert_eq!(
                helper_under(path, prefix),
                crate::ticket_planner::under(path, prefix),
                "divergent under for {path:?}"
            );
        }
    }
}
