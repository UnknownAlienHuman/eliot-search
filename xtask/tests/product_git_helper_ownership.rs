//! Focused ownership and parity proof for the product-owned Git helper.
//!
//! #214 requires the conservative repository-path grammar and the planner file-byte ceiling to
//! live in product-owned tooling. These tests pin that ownership at the real reader seam, and
//! verify the legacy planner aliases stay behaviour-identical delegations.

use std::path::{Path, PathBuf};
use std::process::Command;

use xtask::git_tree::GitTree;
use xtask::product_git_helper::{helper_safe_path, helper_under, HELPER_FILE_BYTE_CEILING};
use xtask::ticket_planner::{safe_path, under, PLANNER_FILE_BYTE_CEILING};

/// Bounded matrix over the closed safe-path grammar, including its boundary cases.
#[test]
fn safe_path_matrix_covers_the_grammar_boundaries() {
    let accepted = [
        "Cargo.toml",
        "crates/search-contracts/src/lib.rs",
        "docs/adr/0005.md",
        "swarm/crates.toml",
        "a/./b",
        "crates/search-contracts-1/src/lib.rs",
        "A_B.C",
    ];
    for case in accepted {
        assert!(helper_safe_path(case), "must accept {case}");
    }
    let refused = [
        "",
        "/etc/passwd",
        "a//b",
        "a/../b",
        "..",
        "go/src/../../lib.rs",
        "swarm/../crates.toml",
        "crates/search contracts/src/lib.rs",
        "crates/src/lib.rs;rm -rf",
        "crates//../xtask/src/lib.rs",
    ];
    for case in refused {
        assert!(!helper_safe_path(case), "must refuse {case}");
    }
    // A backslash byte is not in the accepted character class, so such a path is unsafe.
    let backslash = format!("crates{}src{}lib.rs", char::from(92u8), char::from(92u8));
    assert!(!helper_safe_path(&backslash), "must refuse {backslash}");
}

/// Legacy planner names remain thin, behaviour-identical delegations of the product helper.
#[test]
fn legacy_planner_aliases_delegate_to_the_helper() {
    for case in [
        "Cargo.toml",
        "crates/search-contracts/src/lib.rs",
        "swarm/tickets/a.toml",
        "",
        "/etc/passwd",
        "a//b",
        "a/../b",
        "a/./b",
        "..",
        "crates/search contracts/src/lib.rs",
    ] {
        assert_eq!(helper_safe_path(case), safe_path(case), "safe_path drift at {case:?}");
    }
    for (path, prefix) in [
        ("docs", "docs"),
        ("docs/adr/0005.md", "docs"),
        ("docs-archive", "docs"),
        ("do", "docs"),
        ("", ""),
    ] {
        assert_eq!(helper_under(path, prefix), under(path, prefix), "under drift at {path:?}");
    }
    assert_eq!(PLANNER_FILE_BYTE_CEILING, HELPER_FILE_BYTE_CEILING);
}

/// The immutable Git reader refuses unsafe paths at its real read seam.
#[test]
fn git_tree_entry_reads_refuse_unsafe_paths() {
    let root = repository_root();
    let format = git_text(&root, &["rev-parse", "--show-object-format"]);
    let head = git_text(&root, &["rev-parse", "HEAD"]);
    let view = GitTree::open(&root, &format!("{format}:{head}")).expect("open immutable tree");

    // One safe committed path resolves to exactly one entry.
    assert!(
        view.entry("xtask/src/lib.rs")
            .expect("entry read is not a repository error")
            .is_some(),
        "safe committed path must resolve"
    );

    // Unsafe paths are refused by the product-owned grammar before Git is consulted.
    for refused in [
        "",
        "/etc/passwd",
        "xtask/../xtask/src/lib.rs",
        "xtask//src/lib.rs",
        "..",
        &String::from(char::from(92u8)),
    ] {
        assert!(
            view.entry(refused)
                .expect("entry read is not a repository error")
                .is_none(),
            "unsafe path must not resolve: {refused:?}"
        );
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under the repository root")
        .to_owned()
}

fn git_text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
            .output()
        .expect("git executes");
    assert!(output.status.success(), "git command failed");
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}
