use std::collections::BTreeSet;

pub(super) fn vendor_identifiers(
    code: &str,
    vendor_module: &str,
) -> BTreeSet<String> {
    let mut identifiers = BTreeSet::from([vendor_module.to_owned()]);
    let import_statements = collect_statements(code, is_import_start);
    for statement in &import_statements {
        if statement.contains(vendor_module) {
            add_identifiers_after_vendor(
                statement,
                vendor_module,
                &mut identifiers,
            );
        }
    }

    let type_aliases = collect_statements(code, is_type_alias_start);
    loop {
        let mut changed = false;
        for statement in &type_aliases {
            let Some((left, right)) = statement.split_once('=') else {
                continue;
            };
            if !contains_any_identifier(right, &identifiers) {
                continue;
            }
            let Some(alias) = type_alias_name(left) else {
                continue;
            };
            changed |= identifiers.insert(alias.to_owned());
        }
        if !changed {
            break;
        }
    }
    identifiers
}

fn is_import_start(line: &str) -> bool {
    line.starts_with("use ")
        || line.starts_with("pub use ")
        || line.starts_with("extern crate ")
        || line.starts_with("pub extern crate ")
}

fn is_type_alias_start(line: &str) -> bool {
    line.starts_with("type ")
        || line.starts_with("pub type ")
        || (line.starts_with("pub(") && line.contains(") type "))
}

fn collect_statements(
    code: &str,
    starts: fn(&str) -> bool,
) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if current.is_empty() {
            if !starts(trimmed) {
                continue;
            }
        } else {
            current.push(' ');
        }
        current.push_str(trimmed);
        if trimmed.ends_with(';') {
            statements.push(std::mem::take(&mut current));
        }
    }
    statements
}

fn add_identifiers_after_vendor(
    statement: &str,
    vendor_module: &str,
    identifiers: &mut BTreeSet<String>,
) {
    let Some(index) = statement.find(vendor_module) else {
        return;
    };
    let tail = &statement[index + vendor_module.len()..];
    for identifier in rust_identifiers(tail) {
        if !matches!(
            identifier.as_str(),
            "as"
                | "crate"
                | "extern"
                | "pub"
                | "self"
                | "super"
                | "use"
                | "r"
        ) {
            identifiers.insert(identifier);
        }
    }
}

fn rust_identifiers(text: &str) -> Vec<String> {
    let mut identifiers = Vec::new();
    let mut current = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            current.push(char::from(byte));
        } else if !current.is_empty() {
            identifiers.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        identifiers.push(current);
    }
    identifiers
}

fn type_alias_name(left: &str) -> Option<&str> {
    let type_offset = left.find("type ")? + "type ".len();
    let tail = left[type_offset..].trim_start();
    let end = tail
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_')
        })
        .unwrap_or(tail.len());
    (end > 0).then_some(&tail[..end])
}

pub(super) fn contains_any_identifier(
    text: &str,
    identifiers: &BTreeSet<String>,
) -> bool {
    identifiers
        .iter()
        .any(|identifier| contains_identifier(text, identifier))
}

fn contains_identifier(text: &str, identifier: &str) -> bool {
    text.match_indices(identifier).any(|(start, _)| {
        let end = start + identifier.len();
        let before = text[..start].bytes().next_back();
        let after = text[end..].bytes().next();
        before.is_none_or(|byte| !is_identifier_byte(byte))
            && after.is_none_or(|byte| !is_identifier_byte(byte))
    })
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
