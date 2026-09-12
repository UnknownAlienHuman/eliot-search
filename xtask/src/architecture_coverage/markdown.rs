//! Markdown extraction for architecture coverage.

use std::collections::{BTreeMap, BTreeSet};

const RESERVED_SIGNATURE_WORDS: [&str; 6] =
    ["if", "for", "while", "match", "loop", "return"];

pub(super) fn fenced_blocks(text: &str, language: Option<&str>) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut rest = text;
    loop {
        let Some(marker) = rest.find("```") else {
            break;
        };
        let after_marker = &rest[marker + 3..];
        let Some(line_end) = after_marker.find('\n') else {
            break;
        };
        let tag = after_marker[..line_end].trim();
        let content = &after_marker[line_end + 1..];
        let Some(end) = content.find("```") else {
            break;
        };
        if language.is_none_or(|expected| tag == expected) {
            blocks.push(content[..end].to_owned());
        }
        rest = &content[end + 3..];
    }
    blocks
}

pub(super) fn operation_names(text: &str) -> BTreeSet<String> {
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
    for block in fenced_blocks(text, None) {
        for line in block.lines() {
            if let Some(name) = callable_line(line) {
                names.insert(name.to_owned());
            }
        }
    }
    for reserved in RESERVED_SIGNATURE_WORDS {
        names.remove(reserved);
    }
    names
}

pub(super) fn architecture_sections(text: &str) -> BTreeMap<String, String> {
    let mut sections = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("## ") else {
            continue;
        };
        let Some((id, heading)) = rest.split_once(". ") else {
            continue;
        };
        if id.strip_prefix('S').is_some_and(all_ascii_digits) {
            sections.insert(id.to_owned(), heading.trim().to_owned());
        }
    }
    sections
}

pub(super) fn capability_cells(text: &str) -> BTreeMap<String, String> {
    let mut cells = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(body) = trimmed.strip_prefix('|') else {
            continue;
        };
        let Some((first, _)) = body.split_once('|') else {
            continue;
        };
        let first = first.trim();
        let Some((id, name)) = first.split_once(char::is_whitespace) else {
            continue;
        };
        if id.len() == 3
            && id.starts_with('C')
            && all_ascii_digits(&id[1..])
            && !name.trim().is_empty()
        {
            cells.insert(id.to_owned(), name.trim().to_owned());
        }
    }
    cells
}

pub(super) fn invariant_ids(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some((id, _)) = trimmed.split_once(':') else {
            continue;
        };
        if id.len() == 6
            && id.starts_with("INV-")
            && all_ascii_digits(&id[4..])
        {
            result.insert(id.to_owned());
        }
    }
    result
}

pub(super) fn delivery_ids(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("### ") else {
            continue;
        };
        let Some((id, suffix)) = rest.split_once(char::is_whitespace) else {
            continue;
        };
        let suffix = suffix.trim_start();
        if id.len() == 3
            && id.starts_with('P')
            && all_ascii_digits(&id[1..])
            && (suffix.starts_with('—') || suffix.starts_with('-'))
        {
            result.insert(id.to_owned());
        }
    }
    result
}

pub(super) fn port_methods(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(name) = port_heading(line) {
            result.entry(name.to_owned()).or_insert_with(Vec::new);
            current = Some(name.to_owned());
            continue;
        }
        let Some(port) = current.as_ref() else {
            continue;
        };
        let Some(rest) = line.strip_prefix("- `") else {
            continue;
        };
        let Some((name, _)) = rest.split_once('(') else {
            continue;
        };
        if valid_operation_name(name) {
            result.entry(port.clone()).or_default().push(name.to_owned());
        }
    }
    result
}

pub(super) fn top_level_yaml_labels(text: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    for block in fenced_blocks(text, Some("yaml")) {
        for line in block.lines() {
            let Some(colon) = line.find(':') else {
                continue;
            };
            let candidate = &line[..colon];
            let after = &line[colon + 1..];
            if candidate
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic())
                && candidate.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b'_' | b'<' | b'>' | b'@' | b',' | b'.' | b'-')
                })
                && (after.is_empty()
                    || after
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_whitespace))
            {
                labels.insert(candidate.to_owned());
            }
        }
    }
    labels
}

pub(super) fn exact_type_registry_symbols(
    type_registry: &str,
) -> Result<BTreeSet<String>, String> {
    let mut symbols = BTreeSet::new();

    let bounds = section_between(
        type_registry,
        "## Bounds and collections",
        "## Opaque and display wrappers",
    )?;
    for block in fenced_blocks(bounds, Some("text")) {
        for line in block.lines() {
            if let Some(name) = bounded_type_prefix(line.trim()) {
                symbols.insert(name.to_owned());
            }
        }
    }
    symbols.extend(top_level_yaml_labels(bounds));

    let opaque = section_between(
        type_registry,
        "## Opaque and display wrappers",
        "## Identity and reference registry",
    )?;
    for line in opaque.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        if let Some((name, _)) = rest.split_once("` |") {
            symbols.insert(name.to_owned());
        }
    }

    let identity = section_between(
        type_registry,
        "## Identity and reference registry",
        "## Baseline semantic registries",
    )?;
    let identity_blocks = fenced_blocks(identity, Some("text"));
    if identity_blocks.len() < 3 {
        return Err(
            "TYPE_REGISTRY identity section must contain three text registries"
                .to_owned(),
        );
    }
    for block in identity_blocks.iter().take(3) {
        for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
            for token in line.split(',') {
                let token = token.trim().trim_end_matches('.');
                if !token.is_empty() {
                    symbols.insert(token.to_owned());
                }
            }
        }
    }
    symbols.extend(top_level_yaml_labels(identity));

    let coverage_heading = if type_registry.contains("## Coverage and freshness records") {
        "## Coverage and freshness records"
    } else {
        "## Coverage records"
    };
    let semantic = section_between(
        type_registry,
        "## Baseline semantic registries",
        coverage_heading,
    )?;
    for block in fenced_blocks(semantic, Some("text")) {
        for line in block.lines() {
            let trimmed = line.trim();
            let Some((name, _)) = trimmed.split_once('=') else {
                continue;
            };
            let name = name.trim();
            if valid_type_name(name) {
                symbols.insert(name.to_owned());
            }
        }
    }
    if semantic.contains("`EntityKind`") {
        symbols.insert("EntityKind".to_owned());
    }

    let port_heading = if type_registry.contains("## Port-support records") {
        "## Port-support records"
    } else {
        "## Port support records — owned by `search-ports`"
    };
    let coverage = section_between(type_registry, coverage_heading, port_heading)?;
    symbols.extend(top_level_yaml_labels(coverage));

    let end_heading = if type_registry.contains("## Ownership and visibility summary") {
        "## Ownership and visibility summary"
    } else {
        "## New-type rule"
    };
    let port_support = section_between(type_registry, port_heading, end_heading)?;
    symbols.extend(top_level_yaml_labels(port_support));
    Ok(symbols)
}

pub(super) fn recipe_ids(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for line in text.lines() {
        let mut candidate = line.trim();
        if let Some(rest) = candidate.strip_prefix("- ") {
            candidate = rest.trim();
        }
        candidate = candidate.trim_end_matches(':').trim();
        if let Some(base) = candidate.strip_suffix("@1")
            && valid_operation_name(base)
        {
            result.insert(candidate.to_owned());
        }
    }
    result
}

pub(super) fn reason_codes(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && line
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
                && line.as_bytes()[0].is_ascii_uppercase()
        })
        .map(str::to_owned)
        .collect()
}

pub(super) fn normalize_type_name(name: &str) -> &str {
    name.split_once('<').map_or(name, |(base, _)| base)
}

fn section_between<'a>(
    text: &'a str,
    start: &str,
    end: &str,
) -> Result<&'a str, String> {
    let start_index = text
        .find(start)
        .ok_or_else(|| format!("missing section heading: {start}"))?
        + start.len();
    let end_index = text[start_index..]
        .find(end)
        .map(|offset| start_index + offset)
        .ok_or_else(|| format!("missing section heading: {end}"))?;
    Ok(&text[start_index..end_index])
}

fn bounded_type_prefix(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("Bounded")?;
    let base_end = rest
        .bytes()
        .position(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'))
        .unwrap_or(rest.len());
    if base_end == 0 {
        return None;
    }
    let mut end = "Bounded".len() + base_end;
    if line.as_bytes().get(end) == Some(&b'<') {
        let close = line[end..].find('>')?;
        end += close + 1;
    }
    Some(&line[..end])
}

fn port_heading(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("### `")?;
    let (name, suffix) = rest.split_once('`')?;
    if !name.ends_with("Port")
        || !name.bytes().next().is_some_and(|byte| byte.is_ascii_alphabetic())
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return None;
    }
    let suffix = suffix.trim();
    (suffix.is_empty() || suffix.starts_with('—') || suffix.starts_with('-'))
        .then_some(name)
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
            !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        })
        .unwrap_or(bytes.len());
    Some((&text[..end], &text[end..]))
}

fn valid_operation_name(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_type_name(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_uppercase)
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn all_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{architecture_sections, capability_cells, operation_names, recipe_ids};

    #[test]
    fn extracts_closed_markdown_identities() {
        let operations = operation_names(
            "## `open(root)`\nSee `read(id)`.\n```rust\npub async fn write(x: X)\n```\n",
        );
        assert!(operations.contains("open"));
        assert!(operations.contains("read"));
        assert!(operations.contains("write"));

        assert_eq!(
            architecture_sections("## S0. Intro\n## S1. Next\n").len(),
            2
        );
        assert_eq!(capability_cells("| C00 Exact lookup | x |\n").len(), 1);
        assert!(recipe_ids("- locate@1:\ncompare@1\n").contains("locate@1"));
    }
}
