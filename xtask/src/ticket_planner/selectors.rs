//! Closed registry-selector resolution over pre-loaded documents.

use serde_json::Value;

/// The single row of an array-of-tables matching `key == expected`.
#[must_use]
pub fn one_table<'a>(
    rows: &'a Value,
    key: &str,
    expected: &str,
) -> Option<&'a Value> {
    let rows = rows.as_array()?;
    let mut hits = rows
        .iter()
        .filter(|row| row.get(key).and_then(Value::as_str) == Some(expected));
    let hit = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(hit)
}

fn count_occurrences(values: Option<&Value>, target: &str) -> usize {
    values.and_then(Value::as_array).map_or(0, |items| {
        items
            .iter()
            .filter(|item| item.as_str() == Some(target))
            .count()
    })
}

fn selector_name_valid(value: &str) -> bool {
    let mut characters = value.chars();
    if !matches!(characters.next(), Some(character) if character.is_ascii_lowercase()) {
        return false;
    }
    characters.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'
    })
}

fn stage_id_valid(value: &str) -> bool {
    if value == "W10" {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() == 2 && bytes[0] == b'W' && bytes[1].is_ascii_digit()
}

fn bracketed<'a>(expression: &'a str, prefix: &str) -> Option<&'a str> {
    expression.strip_prefix(prefix)?.strip_suffix(']')
}

/// Pre-loaded registry documents for [`resolve_selector`].
pub struct SelectorDocs<'a> {
    /// Parsed `swarm/crates.toml`.
    pub crates: Option<&'a Value>,
    /// Parsed `swarm/function-packets.toml`.
    pub functions: Option<&'a Value>,
    /// Parsed `swarm/stages.toml`.
    pub stages: Option<&'a Value>,
    /// Parsed `swarm/launch-state.toml`.
    pub launch: Option<&'a Value>,
}

/// Selector resolution outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorStatus {
    /// Selector resolved exactly once.
    Ok,
    /// Selector expression or registry path is unsupported.
    Unsupported,
    /// Registry resolved zero or multiple times.
    NotUnique,
}

impl SelectorStatus {
    /// Stable planner status token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Unsupported => "UNSUPPORTED",
            Self::NotUnique => "NOT_UNIQUE",
        }
    }
}

/// Resolves one selector over pre-loaded documents.
#[must_use]
pub fn resolve_selector(
    docs: &SelectorDocs<'_>,
    selector: &str,
    package: &str,
) -> (SelectorStatus, &'static str) {
    use SelectorStatus::{NotUnique, Unsupported};
    let Some((path, expression)) = selector.split_once("::") else {
        return (Unsupported, "missing :: separator");
    };
    if !matches!(
        path,
        "swarm/crates.toml"
            | "swarm/function-packets.toml"
            | "swarm/stages.toml"
            | "swarm/launch-state.toml"
    ) {
        return (
            Unsupported,
            "registry path is not in the closed selector set",
        );
    }
    let document = match path {
        "swarm/crates.toml" => docs.crates,
        "swarm/function-packets.toml" => docs.functions,
        "swarm/stages.toml" => docs.stages,
        _ => docs.launch,
    };
    let Some(document) = document else {
        return (NotUnique, "registry path is missing or invalid");
    };

    if let Some(outcome) = resolve_bracketed(path, document, expression, package) {
        return outcome;
    }
    if let Some(outcome) = resolve_launchish(path, document, expression, package) {
        return outcome;
    }
    (Unsupported, "unsupported selector expression")
}

fn resolve_bracketed(
    path: &str,
    document: &Value,
    expression: &str,
    package: &str,
) -> Option<(SelectorStatus, &'static str)> {
    use SelectorStatus::{NotUnique, Ok, Unsupported};
    if let Some(name) = bracketed(expression, "package[name=") {
        if !selector_name_valid(name) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/crates.toml" || name != package {
            return Some((Unsupported, "package selector path or identity mismatch"));
        }
        let hit = document
            .get("package")
            .and_then(|rows| one_table(rows, "name", package));
        return Some(match hit {
            Some(_) => (Ok, "one package row"),
            None => (NotUnique, "package selector did not resolve exactly once"),
        });
    }
    if let Some(name) = bracketed(expression, "foundation[package=") {
        if !selector_name_valid(name) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/function-packets.toml" || name != package {
            return Some((Unsupported, "foundation selector path or identity mismatch"));
        }
        let hit = document
            .get("foundation")
            .and_then(|rows| one_table(rows, "package", package));
        return Some(match hit {
            Some(_) => (Ok, "one foundation row"),
            None => (
                NotUnique,
                "foundation selector did not resolve exactly once",
            ),
        });
    }
    if let Some(id) = bracketed(expression, "stage[id=") {
        if !stage_id_valid(id) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/stages.toml" || id != "W0" {
            return Some((Unsupported, "stage selector path or stage mismatch"));
        }
        let row = document
            .get("stage")
            .and_then(|rows| one_table(rows, "id", "W0"));
        let Some(row) = row else {
            return Some((NotUnique, "stage selector did not resolve exactly once"));
        };
        if count_occurrences(row.get("packages"), package) != 1 {
            return Some((
                NotUnique,
                "selected stage does not contain package exactly once",
            ));
        }
        return Some((Ok, "one W0 stage row containing package"));
    }
    None
}

fn resolve_launchish(
    path: &str,
    document: &Value,
    expression: &str,
    package: &str,
) -> Option<(SelectorStatus, &'static str)> {
    use SelectorStatus::{NotUnique, Ok, Unsupported};
    if let Some(rest) = expression.strip_suffix(']') {
        for key in ["authorized_packages", "conditional_packages"] {
            if let Some(inner) = rest.strip_prefix(&format!("{key}[")) {
                if !selector_name_valid(inner) {
                    return Some((Unsupported, "unsupported selector expression"));
                }
                return Some(launch_membership_at_path(
                    path, document, key, inner, package,
                ));
            }
        }
    }
    if let Some(name) = expression.strip_prefix("conditional_activation.") {
        if !selector_name_valid(name) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/launch-state.toml" || name != package {
            return Some((
                Unsupported,
                "conditional activation path or package mismatch",
            ));
        }
        let table = document.get("conditional_activation");
        if table
            .and_then(|value| value.get(package))
            .is_some_and(Value::is_table)
        {
            return Some((Ok, "one conditional activation table"));
        }
        return Some((
            NotUnique,
            "conditional activation did not resolve exactly once",
        ));
    }
    None
}

/// Resolves one launch membership selector.
#[must_use]
pub fn launch_membership_at_path(
    path: &str,
    document: &Value,
    key: &str,
    name: &str,
    package: &str,
) -> (SelectorStatus, &'static str) {
    use SelectorStatus::{NotUnique, Ok, Unsupported};
    if path != "swarm/launch-state.toml" || name != package {
        return (Unsupported, "launch selector path or package mismatch");
    }
    if count_occurrences(document.get(key), package) == 1 {
        (Ok, "one launch membership")
    } else {
        (NotUnique, "launch membership did not resolve exactly once")
    }
}
