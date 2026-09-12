//! Operation registry and source-derived function closure.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use toml::Value;

use super::{boolean, child, integer, require, string};

const FOUNDATION: [&str; 3] = ["search-contracts", "search-domain", "search-ports"];
const RESERVED: [&str; 6] = ["if", "for", "while", "match", "loop", "return"];

pub(super) fn validate(
    root: &Path,
    packages: &BTreeMap<String, Value>,
    foundations: &BTreeMap<String, Value>,
    functions: &BTreeMap<String, Value>,
    operations: &Value,
    errors: &mut Vec<String>,
) -> usize {
    require(
        errors,
        string(operations, "status")
            == Some("SOURCE_DERIVED_OPERATION_OWNERSHIP_CLOSED_NOT_IMPLEMENTED"),
        "operation registry status changed",
    );
    require(
        errors,
        string(operations, "function_registry")
            == Some("swarm/function-packets.toml"),
        "operation registry function source mismatch",
    );
    require(
        errors,
        string(operations, "module_registry")
            == Some("swarm/module-packets.toml"),
        "operation registry module source mismatch",
    );
    require(
        errors,
        integer(operations, "package_count") == Some(45),
        "operation registry package_count mismatch",
    );
    require(
        errors,
        integer(operations, "foundation_package_count") == Some(3),
        "operation registry foundation count mismatch",
    );
    require(
        errors,
        integer(operations, "package_function_source_count") == Some(42),
        "operation registry function-source count mismatch",
    );
    require(
        errors,
        string(operations, "operation_count") == Some("DERIVED_BY_VALIDATOR"),
        "operation count must remain source-derived",
    );
    require(
        errors,
        boolean(operations, "implementation_authorized_by_this_registry")
            == Some(false),
        "operation registry authorizes implementation",
    );

    validate_identity(operations, errors);
    validate_extraction(operations, errors);
    validate_ownership(operations, errors);
    validate_invariants(operations, errors);
    validate_foundations(foundations, operations, errors);
    validate_function_sources(root, packages, functions, errors)
}

fn validate_identity(operations: &Value, errors: &mut Vec<String>) {
    require(
        errors,
        child(operations, "identity", "format").and_then(Value::as_str)
            == Some("<package>::<operation>"),
        "qualified operation identity format changed",
    );
    require(
        errors,
        child(
            operations,
            "identity",
            "package_qualified_identity_required",
        )
        .and_then(Value::as_bool)
            == Some(true),
        "package-qualified operation IDs disabled",
    );
    require(
        errors,
        child(
            operations,
            "identity",
            "duplicate_unqualified_operation_names_allowed",
        )
        .and_then(Value::as_bool)
            == Some(true),
        "unqualified operation collision rule changed",
    );
    require(
        errors,
        child(
            operations,
            "identity",
            "duplicate_package_qualified_operation_names_allowed",
        )
        .and_then(Value::as_bool)
            == Some(false),
        "qualified operation collisions allowed",
    );
}

fn validate_extraction(operations: &Value, errors: &mut Vec<String>) {
    require(
        errors,
        child(
            operations,
            "extraction",
            "minimum_operations_per_nonfoundation_package",
        )
        .and_then(Value::as_integer)
            == Some(1),
        "minimum operation count weakened",
    );
    require(
        errors,
        child(
            operations,
            "extraction",
            "ignore_test_fixture_and_example_blocks",
        )
        .and_then(Value::as_bool)
            == Some(true),
        "fixture/example exclusion disabled",
    );
    require(
        errors,
        child(
            operations,
            "extraction",
            "ignore_Rust_trait_methods_in_package_FUNCTIONS",
        )
        .and_then(Value::as_bool)
            == Some(false),
        "trait methods excluded from ownership",
    );
}

fn validate_ownership(operations: &Value, errors: &mut Vec<String>) {
    require(
        errors,
        child(operations, "ownership", "operation_owner_source")
            .and_then(Value::as_str)
            == Some("function_registry_package"),
        "operation owner source changed",
    );
    require(
        errors,
        child(operations, "ownership", "public_entry_module_source")
            .and_then(Value::as_str)
            == Some("module_registry_package"),
        "public entry source changed",
    );
    for key in [
        "every_operation_enters_through_public_entry_module",
        "internal_delegation_must_remain_within_declared_package_modules",
        "operation_source_must_be_package_local",
    ] {
        require(
            errors,
            child(operations, "ownership", key).and_then(Value::as_bool)
                == Some(true),
            format!("operation ownership invariant disabled: {key}"),
        );
    }
    require(
        errors,
        child(
            operations,
            "ownership",
            "cross_package_operation_implementation_allowed",
        )
        .and_then(Value::as_bool)
            == Some(false),
        "cross-package operation implementation allowed",
    );
}

fn validate_invariants(operations: &Value, errors: &mut Vec<String>) {
    for key in [
        "missing_function_source_blocks_merge",
        "function_source_without_registered_package_blocks_merge",
        "registered_package_without_discovered_operation_blocks_merge",
        "operation_without_declared_public_entry_blocks_merge",
        "operation_implementation_outside_package_write_scope_blocks_merge",
    ] {
        require(
            errors,
            child(operations, "invariants", key).and_then(Value::as_bool)
                == Some(true),
            format!("operation merge guard disabled: {key}"),
        );
    }
    require(
        errors,
        child(operations, "invariants", "placeholder_success_allowed")
            .and_then(Value::as_bool)
            == Some(false),
        "placeholder success allowed",
    );
}

fn validate_foundations(
    foundations: &BTreeMap<String, Value>,
    operations: &Value,
    errors: &mut Vec<String>,
) {
    let expected: BTreeSet<String> = FOUNDATION.iter().map(|value| (*value).to_owned()).collect();
    let actual: BTreeSet<String> = foundations.keys().cloned().collect();
    require(
        errors,
        actual == expected,
        "foundation function registry mismatch",
    );

    for (package, source, entry) in [
        (
            "search-contracts",
            "docs/contracts/p00/README.md",
            "lib",
        ),
        (
            "search-domain",
            "docs/contracts/p00/SUPPORT_SCHEMAS.md",
            "lib",
        ),
        (
            "search-ports",
            "docs/contracts/p00/PORT_OPERATIONS.md",
            "lib",
        ),
    ] {
        let registry_row = foundations.get(package);
        require(
            errors,
            row_string(registry_row, "primary_contract") == Some(source),
            format!("{package}: foundation contract source mismatch"),
        );
        let key = package.replace('-', "_");
        let operation_row = operations
            .get("foundation")
            .and_then(Value::as_table)
            .and_then(|table| table.get(&key));
        require(
            errors,
            row_string(operation_row, "package") == Some(package),
            format!("{package}: operation foundation package mismatch"),
        );
        require(
            errors,
            row_string(operation_row, "source") == Some(source),
            format!("{package}: operation foundation source mismatch"),
        );
        require(
            errors,
            row_string(operation_row, "public_entry_module") == Some(entry),
            format!("{package}: operation public entry mismatch"),
        );
    }
}

fn validate_function_sources(
    root: &Path,
    packages: &BTreeMap<String, Value>,
    functions: &BTreeMap<String, Value>,
    errors: &mut Vec<String>,
) -> usize {
    let expected: BTreeSet<String> = packages
        .keys()
        .filter(|package| !FOUNDATION.contains(&package.as_str()))
        .cloned()
        .collect();
    let actual: BTreeSet<String> = functions.keys().cloned().collect();
    require(
        errors,
        actual == expected,
        "non-foundation function package set mismatch",
    );

    let mut qualified = BTreeSet::new();
    let mut count = 0_usize;
    for (package, row) in functions {
        let path = row.get("functions").and_then(Value::as_str);
        require(
            errors,
            path.is_some_and(|relative| root.join(relative).is_file()),
            format!("{package}: function source missing"),
        );
        let package_path = packages
            .get(package)
            .and_then(|package_row| package_row.get("path"))
            .and_then(Value::as_str);
        require(
            errors,
            path.zip(package_path).is_some_and(|(relative, owner)| {
                relative.starts_with(&format!("{owner}/"))
            }),
            format!("{package}: function source outside package"),
        );
        let Some(relative) = path else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(root.join(relative)) else {
            continue;
        };
        let names = operation_names(&text);
        require(
            errors,
            !names.is_empty(),
            format!("{package}: no source-derived operations"),
        );
        for name in names {
            let identity = format!("{package}::{name}");
            require(
                errors,
                qualified.insert(identity.clone()),
                format!("duplicate qualified operation {identity}"),
            );
            count = count.saturating_add(1);
        }
    }
    count
}

fn row_string<'a>(row: Option<&'a Value>, key: &str) -> Option<&'a str> {
    row?.get(key)?.as_str()
}

fn operation_names(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in text.lines() {
        if let Some(name) = heading_operation(line) {
            names.insert(name.to_owned());
        }
    }
    for segment in inline_code_segments(text) {
        if let Some((name, rest)) = identifier_prefix(segment)
            && rest.starts_with('(')
        {
            names.insert(name.to_owned());
        }
    }
    for block in fenced_blocks(text) {
        for line in block.lines() {
            if let Some(name) = callable_line(line) {
                names.insert(name.to_owned());
            }
        }
    }
    for reserved in RESERVED {
        names.remove(reserved);
    }
    names
}

fn heading_operation(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let hashes = bytes.iter().take_while(|byte| **byte == b'#').count();
    if !(2..=3).contains(&hashes) {
        return None;
    }
    let mut rest = &line[hashes..];
    if rest.is_empty() || !rest.as_bytes()[0].is_ascii_whitespace() {
        return None;
    }
    rest = rest.trim_start_matches(char::is_whitespace);
    rest = rest.strip_prefix('`')?;
    identifier_prefix(rest).map(|(name, _)| name)
}

fn inline_code_segments(text: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find('`') else {
            break;
        };
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else {
            break;
        };
        segments.push(&after[..end]);
        rest = &after[end + 1..];
    }
    segments
}

fn fenced_blocks(text: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find("```") else {
            break;
        };
        let after_marker = &rest[start + 3..];
        let Some(line_end) = after_marker.find('\n') else {
            break;
        };
        let content = &after_marker[line_end + 1..];
        let Some(end) = content.find("```") else {
            break;
        };
        blocks.push(&content[..end]);
        rest = &content[end + 3..];
    }
    blocks
}

fn callable_line(line: &str) -> Option<&str> {
    let mut rest = line;
    if let Some(after) = strip_keyword(rest, "pub") {
        rest = after;
    }
    if let Some(after) = strip_keyword(rest, "async") {
        rest = after;
    }
    if let Some(after) = strip_keyword(rest, "fn") {
        rest = after;
    }
    let (name, tail) = identifier_prefix(rest)?;
    tail.trim_start_matches(char::is_whitespace)
        .starts_with('(')
        .then_some(name)
}

fn strip_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let tail = text.strip_prefix(keyword)?;
    if tail.is_empty() || !tail.as_bytes()[0].is_ascii_whitespace() {
        return None;
    }
    Some(tail.trim_start_matches(char::is_whitespace))
}

fn identifier_prefix(text: &str) -> Option<(&str, &str)> {
    let bytes = text.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_lowercase) {
        return None;
    }
    let end = bytes
        .iter()
        .position(|byte| {
            !(byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || *byte == b'_')
        })
        .unwrap_or(bytes.len());
    Some((&text[..end], &text[end..]))
}

#[cfg(test)]
mod tests {
    use super::operation_names;

    #[test]
    fn extracts_heading_inline_and_fenced_operations() {
        let text = concat!(
            "## `open_store(root)`\n",
            "See `resolve_item(id)` and `if(x)`.\n",
            "```rust\n",
            "pub async fn execute_query(input: Query) -> Result<()>\n",
            "helper(value)\n",
            "```\n",
        );
        let names = operation_names(text);
        assert!(names.contains("open_store"));
        assert!(names.contains("resolve_item"));
        assert!(names.contains("execute_query"));
        assert!(names.contains("helper"));
        assert!(!names.contains("if"));
    }
}
