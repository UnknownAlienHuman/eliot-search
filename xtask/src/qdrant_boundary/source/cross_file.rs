use std::collections::BTreeSet;

use super::imports::{contains_any_identifier, vendor_identifiers};
use super::lexer::code_only;
use super::surface::find_public_vendor_surfaces;

/// One bridge source retained from the bounded repository walk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BridgeSource {
    relative: String,
    source: String,
}

impl BridgeSource {
    pub(super) fn new(relative: String, source: String) -> Self {
        Self { relative, source }
    }
}

#[derive(Clone, Debug)]
struct SourceUnit {
    relative: String,
    module: Vec<String>,
    code: String,
}

#[derive(Clone, Debug)]
struct UseStatement {
    line: usize,
    public: bool,
    tree: String,
}

#[derive(Clone, Debug)]
struct UseLeaf {
    path: Vec<String>,
    binding: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedItem {
    module: Vec<String>,
    name: String,
}

/// Finds vendor-tainted aliases imported or publicly re-exported across bridge
/// source files.
///
/// The file-local scanner owns direct SDK references. This pass closes the
/// standard Rust module-path escape where a private alias is defined in one
/// file, then imported into a public signature or re-exported by another file.
pub(super) fn find_cross_file_vendor_surfaces(
    sources: &[BridgeSource],
    bridge_root: &str,
    vendor_module: &str,
) -> Vec<(String, usize)> {
    let units = sources
        .iter()
        .filter_map(|source| {
            let module = module_path(&source.relative, bridge_root)?;
            Some(SourceUnit {
                relative: source.relative.clone(),
                module,
                code: code_only(&source.source),
            })
        })
        .collect::<Vec<_>>();

    let mut tainted_items = BTreeSet::new();
    for unit in &units {
        for binding in direct_tainted_bindings(&unit.code, vendor_module) {
            tainted_items.insert(item_key(&unit.module, &binding));
        }
    }

    let mut findings = BTreeSet::new();
    loop {
        let mut changed = false;
        for unit in &units {
            for statement in collect_use_statements(&unit.code) {
                if !statement.public {
                    continue;
                }
                for leaf in expand_use_tree(&statement.tree) {
                    let Some(resolved) = resolve_item(&unit.module, &leaf.path)
                    else {
                        continue;
                    };
                    if resolved.module == unit.module
                        || !tainted_items
                            .contains(&item_key(&resolved.module, &resolved.name))
                    {
                        continue;
                    }
                    findings.insert((unit.relative.clone(), statement.line));
                    changed |= tainted_items
                        .insert(item_key(&unit.module, &leaf.binding));
                }
            }
        }
        if !changed {
            break;
        }
    }

    for unit in &units {
        let mut imported_tainted_names = BTreeSet::new();
        for statement in collect_use_statements(&unit.code) {
            for leaf in expand_use_tree(&statement.tree) {
                let Some(resolved) = resolve_item(&unit.module, &leaf.path)
                else {
                    continue;
                };
                if resolved.module != unit.module
                    && tainted_items
                        .contains(&item_key(&resolved.module, &resolved.name))
                {
                    imported_tainted_names.insert(resolved.name);
                    imported_tainted_names.insert(leaf.binding);
                }
            }
        }
        if imported_tainted_names.is_empty() {
            continue;
        }
        for line in
            find_public_vendor_surfaces(&unit.code, &imported_tainted_names)
        {
            findings.insert((unit.relative.clone(), line));
        }
    }

    findings.into_iter().collect()
}

fn direct_tainted_bindings(code: &str, vendor_module: &str) -> BTreeSet<String> {
    let identifiers = vendor_identifiers(code, vendor_module);
    let mut bindings = BTreeSet::new();
    let mut vendor_roots = BTreeSet::from([vendor_module.to_owned()]);

    for (_, statement) in collect_semicolon_statements(code) {
        let tokens = identifier_tokens(&statement);
        if is_extern_crate_statement(&tokens) {
            let Some(vendor_index) = tokens
                .iter()
                .position(|token| token == vendor_module)
            else {
                continue;
            };
            let binding = tokens
                .get(vendor_index + 1)
                .filter(|token| token.as_str() == "as")
                .and_then(|_| tokens.get(vendor_index + 2))
                .cloned()
                .unwrap_or_else(|| vendor_module.to_owned());
            vendor_roots.insert(binding.clone());
            bindings.insert(binding);
        }
    }

    for statement in collect_use_statements(code) {
        for leaf in expand_use_tree(&statement.tree) {
            if leaf
                .path
                .first()
                .is_some_and(|root| vendor_roots.contains(root))
            {
                bindings.insert(leaf.binding);
            }
        }
    }

    for (_, statement) in collect_semicolon_statements(code) {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };
        let left_tokens = identifier_tokens(left);
        let Some(type_index) = left_tokens.iter().position(|token| token == "type")
        else {
            continue;
        };
        let Some(alias) = left_tokens.get(type_index + 1) else {
            continue;
        };
        if identifiers.contains(alias)
            && contains_any_identifier(right, &identifiers)
        {
            bindings.insert(alias.clone());
        }
    }

    bindings
}

fn module_path(relative: &str, bridge_root: &str) -> Option<Vec<String>> {
    let prefix = format!("{bridge_root}/src/");
    let local = relative.strip_prefix(&prefix)?;
    let mut components = local.split('/').map(str::to_owned).collect::<Vec<_>>();
    let file = components.pop()?;
    if file == "lib.rs" {
        return components.is_empty().then_some(Vec::new());
    }
    if file == "mod.rs" {
        return Some(components);
    }
    let stem = file.strip_suffix(".rs")?;
    components.push(stem.to_owned());
    Some(components)
}

fn collect_use_statements(code: &str) -> Vec<UseStatement> {
    let mut statements = Vec::new();
    let mut current: Option<(usize, String)> = None;
    let mut pending_public_line: Option<usize> = None;

    for (index, line) in code.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if let Some((_, statement)) = current.as_mut() {
            statement.push(' ');
            statement.push_str(trimmed);
        } else if trimmed == "pub" {
            pending_public_line = Some(line_number);
            continue;
        } else if let Some(public_line) = pending_public_line {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                continue;
            }
            pending_public_line = None;
            if trimmed == "use" || trimmed.starts_with("use ") {
                current = Some((public_line, format!("pub {trimmed}")));
            } else if starts_use_statement(trimmed) {
                current = Some((line_number, trimmed.to_owned()));
            }
        } else if starts_use_statement(trimmed) {
            current = Some((line_number, trimmed.to_owned()));
        }

        let complete = current
            .as_ref()
            .is_some_and(|(_, statement)| statement.contains(';'));
        if !complete {
            continue;
        }
        let (line, statement) = current.take().expect("complete statement");
        if let Some((public, tree)) = strip_use_prefix(&statement) {
            statements.push(UseStatement {
                line,
                public,
                tree: tree.trim_end_matches(';').trim().to_owned(),
            });
        }
    }
    statements
}

fn starts_use_statement(line: &str) -> bool {
    line == "use"
        || line.starts_with("use ")
        || line.starts_with("pub use ")
        || (line.starts_with("pub(") && line.contains(") use "))
}

fn strip_use_prefix(statement: &str) -> Option<(bool, &str)> {
    let statement = statement.trim();
    if let Some(rest) = statement.strip_prefix("pub use ") {
        return Some((true, rest));
    }
    if let Some(rest) = statement.strip_prefix("use ") {
        return Some((false, rest));
    }
    if statement.starts_with("pub(") {
        let end = statement.find(')')?;
        let rest = statement[end + 1..].trim_start().strip_prefix("use ")?;
        return Some((false, rest));
    }
    None
}

fn expand_use_tree(tree: &str) -> Vec<UseLeaf> {
    let mut leaves = Vec::new();
    expand_use_fragment(tree.trim(), &[], &mut leaves);
    leaves
}

fn expand_use_fragment(fragment: &str, prefix: &[String], leaves: &mut Vec<UseLeaf>) {
    let fragment = fragment.trim().trim_end_matches(';').trim();
    if fragment.is_empty() {
        return;
    }
    if let Some(open) = fragment.find('{') {
        let Some(close) = matching_brace(fragment, open) else {
            return;
        };
        let mut next_prefix = prefix.to_vec();
        next_prefix.extend(identifier_tokens(
            fragment[..open].trim_end_matches(':').trim(),
        ));
        for item in split_top_level_commas(&fragment[open + 1..close]) {
            expand_use_fragment(item, &next_prefix, leaves);
        }
        return;
    }

    let tokens = identifier_tokens(fragment);
    if tokens.is_empty() || fragment.contains('*') {
        return;
    }
    let alias_index = tokens.iter().position(|token| token == "as");
    let path_tokens = alias_index.map_or(tokens.as_slice(), |index| &tokens[..index]);
    let alias = alias_index.and_then(|index| tokens.get(index + 1)).cloned();
    if path_tokens.is_empty() {
        return;
    }
    let mut path = prefix.to_vec();
    path.extend(path_tokens.iter().cloned());
    let source_name = path.last().cloned().unwrap_or_default();
    if source_name == "self" {
        return;
    }
    leaves.push(UseLeaf {
        path,
        binding: alias.unwrap_or(source_name),
    });
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, character) in text[open..].char_indices() {
        match character {
            '{' => depth = depth.checked_add(1)?,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_isize;
    let mut start = 0_usize;
    for (index, character) in text.char_indices() {
        match character {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn resolve_item(current_module: &[String], path: &[String]) -> Option<ResolvedItem> {
    let (name, prefix) = path.split_last()?;
    if name == "self" || name == "super" || name == "crate" {
        return None;
    }

    let mut module = Vec::new();
    let mut index = 0_usize;
    match prefix.first().map(String::as_str) {
        Some("crate") => index = 1,
        Some("self") => {
            module.extend_from_slice(current_module);
            index = 1;
        }
        Some("super") => {
            module.extend_from_slice(current_module);
            while prefix.get(index).map(String::as_str) == Some("super") {
                module.pop()?;
                index += 1;
            }
        }
        _ => {}
    }
    module.extend(prefix[index..].iter().cloned());
    Some(ResolvedItem {
        module,
        name: name.clone(),
    })
}

fn item_key(module: &[String], name: &str) -> String {
    let mut key = String::from("crate");
    for segment in module {
        key.push_str("::");
        key.push_str(segment);
    }
    key.push_str("::");
    key.push_str(name);
    key
}

fn collect_semicolon_statements(code: &str) -> Vec<(usize, String)> {
    let mut statements = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (index, line) in code.lines().enumerate() {
        let trimmed = line.trim();
        if let Some((_, statement)) = current.as_mut() {
            statement.push(' ');
            statement.push_str(trimmed);
        } else if starts_semicolon_statement(trimmed) {
            current = Some((index + 1, trimmed.to_owned()));
        }
        if current
            .as_ref()
            .is_some_and(|(_, statement)| statement.contains(';'))
        {
            statements.push(current.take().expect("complete statement"));
        }
    }
    statements
}

fn starts_semicolon_statement(line: &str) -> bool {
    line.starts_with("use ")
        || line.starts_with("pub use ")
        || line.starts_with("extern crate ")
        || line.starts_with("pub extern crate ")
        || line.starts_with("type ")
        || line.starts_with("pub type ")
        || (line.starts_with("pub(") && line.contains(") type "))
}

fn is_extern_crate_statement(tokens: &[String]) -> bool {
    tokens
        .windows(2)
        .any(|pair| pair[0] == "extern" && pair[1] == "crate")
}

fn identifier_tokens(text: &str) -> Vec<String> {
    let mut identifiers = Vec::new();
    let mut current = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == 'r' && characters.peek() == Some(&'#') {
            characters.next();
            continue;
        }
        if character.is_ascii_alphanumeric() || character == '_' {
            current.push(character);
        } else if !current.is_empty() {
            identifiers.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        identifiers.push(current);
    }
    identifiers
}

#[cfg(test)]
mod tests;
