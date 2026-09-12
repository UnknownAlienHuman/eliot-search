//! Repository path, advisory output and context-source fences.

use super::spec::{CONTROL_ROOTS, PLAN_ARTIFACT_ROOT};

/// Repository-relative safe path check.
///
/// Mirrors the planner grammar plus `PurePosixPath` semantics: no absolute,
/// empty or parent segments after dropping single-dot segments.
#[must_use]
pub fn safe_path(value: &str) -> bool {
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
pub fn under(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// Stdout half of advisory output selection.
#[must_use]
pub fn advisory_output_selectable(output: &str) -> bool {
    output == "-"
}

/// Pure path-rule half of advisory output validation.
#[must_use]
pub fn advisory_output_path_valid(output: &str) -> bool {
    let relative = output.replace('\\', "/");
    safe_path(&relative)
        && under(&relative, PLAN_ARTIFACT_ROOT)
        && relative.as_bytes().ends_with(b".json")
        && relative != format!("{PLAN_ARTIFACT_ROOT}/.json")
}

/// Context-source fence predicate.
#[must_use]
pub fn context_source_forbidden(path: &str) -> bool {
    if !safe_path(path)
        || path.starts_with("docs/architecture/")
        || path.starts_with("bins/")
        || is_crate_src(path)
    {
        return true;
    }
    CONTROL_ROOTS.iter().any(|root| under(path, root))
}

fn is_crate_src(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("crates/") else {
        return false;
    };
    rest.split('/').count() >= 3 && rest.split('/').nth(1) == Some("src")
}
