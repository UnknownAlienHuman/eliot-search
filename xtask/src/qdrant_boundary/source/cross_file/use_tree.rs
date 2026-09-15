use std::collections::{BTreeMap, BTreeSet};

use super::super::imports::contains_any_identifier;

const MAX_USE_TREE_DEPTH: usize = 64;
const MAX_USE_LEAVES: usize = 65_536;

#[derive(Clone, Debug)]
pub(super) struct UseStatement {
    pub(super) line: usize,
    pub(super) public: bool,
    pub(super) leaves: Option<Vec<UseLeaf>>,
}

#[derive(Clone, Debug)]
pub(super) struct UseLeaf {
    pub(super) path: Vec<String>,
    pub(super) binding: String,
    pub(super) glob: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct DirectTaint {
    pub(super) bindings: BTreeMap<String, bool>,
    pub(super) wildcard_lines: BTreeSet<usize>,
}

pub(super) fn direct_tainted_bindings(
    code: &str,
    vendor_module: &str,
) -> DirectTaint {
    let use_statements = collect_use_statements(code);
    let mut direct = DirectTaint::default();
    for statement in &use_statements {
        if statement.leaves.is_none() {
            direct.wildcard_lines.insert(statement.line);
        }
    }
    let mut vendor_roots = BTreeSet::from([vendor_module.to_owned()]);
    collect_extern_roots(code, vendor_module, &mut vendor_roots, &mut direct);
    expand_vendor_roots(&use_statements, &mut vendor_roots);
    collect_vendor_imports(&use_statements, &vendor_roots, &mut direct);

    let mut identifiers = vendor_roots;
    identifiers.extend(direct.bindings.keys().cloned());
    collect_vendor_aliases(code, &mut identifiers, &mut direct);
    direct
}

fn collect_extern_roots(
    code: &str,
    vendor_module: &str,
    vendor_roots: &mut BTreeSet<String>,
    direct: &mut DirectTaint,
) {
    for (_, statement) in collect_semicolon_statements(code) {
        let tokens = identifier_tokens(&statement);
        if !is_extern_crate_statement(&tokens) {
            continue;
        }
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
        merge_binding(
            &mut direct.bindings,
            binding,
            statement.trim_start().starts_with("pub extern crate "),
        );
    }
}

fn expand_vendor_roots(
    statements: &[UseStatement],
    vendor_roots: &mut BTreeSet<String>,
) {
    loop {
        let mut changed = false;
        for statement in statements {
            let Some(leaves) = &statement.leaves else {
                continue;
            };
            for leaf in leaves {
                if leaf.glob || !starts_from_vendor_root(leaf, vendor_roots) {
                    continue;
                }
                changed |= vendor_roots.insert(leaf.binding.clone());
            }
        }
        if !changed {
            break;
        }
    }
}

fn collect_vendor_imports(
    statements: &[UseStatement],
    vendor_roots: &BTreeSet<String>,
    direct: &mut DirectTaint,
) {
    for statement in statements {
        let Some(leaves) = &statement.leaves else {
            continue;
        };
        for leaf in leaves {
            if !starts_from_vendor_root(leaf, vendor_roots) {
                continue;
            }
            if leaf.glob {
                direct.wildcard_lines.insert(statement.line);
            } else {
                merge_binding(
                    &mut direct.bindings,
                    leaf.binding.clone(),
                    statement.public,
                );
            }
        }
    }
}

fn starts_from_vendor_root(
    leaf: &UseLeaf,
    vendor_roots: &BTreeSet<String>,
) -> bool {
    leaf.path
        .first()
        .is_some_and(|root| vendor_roots.contains(root))
}

fn collect_vendor_aliases(
    code: &str,
    identifiers: &mut BTreeSet<String>,
    direct: &mut DirectTaint,
) {
    let statements = collect_semicolon_statements(code);
    loop {
        let mut changed = false;
        for (_, statement) in &statements {
            let Some((left, right)) = statement.split_once('=') else {
                continue;
            };
            let left_tokens = identifier_tokens(left);
            let Some(type_index) =
                left_tokens.iter().position(|token| token == "type")
            else {
                continue;
            };
            let Some(alias) = left_tokens.get(type_index + 1) else {
                continue;
            };
            if contains_any_identifier(right, identifiers) {
                merge_binding(
                    &mut direct.bindings,
                    alias.clone(),
                    left.trim_start().starts_with("pub type "),
                );
                changed |= identifiers.insert(alias.clone());
            }
        }
        if !changed {
            break;
        }
    }
}

fn merge_binding(
    bindings: &mut BTreeMap<String, bool>,
    name: String,
    exportable: bool,
) {
    bindings
        .entry(name)
        .and_modify(|current| *current |= exportable)
        .or_insert(exportable);
}

pub(super) fn collect_use_statements(code: &str) -> Vec<UseStatement> {
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
            let tree = tree.trim_end_matches(';').trim();
            statements.push(UseStatement {
                line,
                public,
                leaves: expand_use_tree(tree),
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

fn expand_use_tree(tree: &str) -> Option<Vec<UseLeaf>> {
    let mut leaves = Vec::new();
    expand_use_fragment(tree.trim(), &[], 0, &mut leaves)?;
    Some(leaves)
}

fn expand_use_fragment(
    fragment: &str,
    prefix: &[String],
    depth: usize,
    leaves: &mut Vec<UseLeaf>,
) -> Option<()> {
    if depth > MAX_USE_TREE_DEPTH || leaves.len() > MAX_USE_LEAVES {
        return None;
    }
    let fragment = fragment.trim().trim_end_matches(';').trim();
    if fragment.is_empty() {
        return Some(());
    }
    if fragment == "*" {
        push_leaf(
            leaves,
            UseLeaf {
                path: prefix.to_vec(),
                binding: String::new(),
                glob: true,
            },
        )?;
        return Some(());
    }
    if let Some(before_star) = fragment.strip_suffix('*') {
        let before_star = before_star.trim_end();
        if let Some(glob_prefix) = before_star.strip_suffix("::") {
            let mut path = prefix.to_vec();
            path.extend(identifier_tokens(glob_prefix));
            push_leaf(
                leaves,
                UseLeaf {
                    path,
                    binding: String::new(),
                    glob: true,
                },
            )?;
            return Some(());
        }
    }
    if let Some(open) = fragment.find('{') {
        let close = matching_brace(fragment, open)?;
        if !fragment[close + 1..].trim().is_empty() {
            return None;
        }
        let mut next_prefix = prefix.to_vec();
        next_prefix.extend(identifier_tokens(
            fragment[..open].trim_end_matches(':').trim(),
        ));
        for item in split_top_level_commas(&fragment[open + 1..close])? {
            expand_use_fragment(item, &next_prefix, depth + 1, leaves)?;
        }
        return Some(());
    }

    let tokens = identifier_tokens(fragment);
    if tokens.is_empty() || fragment.contains('*') {
        return None;
    }
    let alias_index = tokens.iter().position(|token| token == "as");
    let path_tokens =
        alias_index.map_or(tokens.as_slice(), |index| &tokens[..index]);
    let alias = alias_index.and_then(|index| tokens.get(index + 1)).cloned();
    if path_tokens.is_empty() {
        return None;
    }
    let mut path = prefix.to_vec();
    path.extend(path_tokens.iter().cloned());
    let source_name = path.last().cloned().unwrap_or_default();
    if source_name == "self" {
        let binding = alias.or_else(|| prefix.last().cloned())?;
        push_leaf(
            leaves,
            UseLeaf {
                path: prefix.to_vec(),
                binding,
                glob: false,
            },
        )?;
        return Some(());
    }
    push_leaf(
        leaves,
        UseLeaf {
            path,
            binding: alias.unwrap_or(source_name),
            glob: false,
        },
    )?;
    Some(())
}

fn push_leaf(leaves: &mut Vec<UseLeaf>, leaf: UseLeaf) -> Option<()> {
    if leaves.len() >= MAX_USE_LEAVES {
        return None;
    }
    leaves.push(leaf);
    Some(())
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

fn split_top_level_commas(text: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0_usize;
    for (index, character) in text.char_indices() {
        match character {
            '{' | '(' | '[' => depth = depth.checked_add(1)?,
            '}' | ')' | ']' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    parts.push(&text[start..]);
    Some(parts)
}

pub(super) fn expand_local_aliases(
    code: &str,
    identifiers: &mut BTreeSet<String>,
) {
    let statements = collect_semicolon_statements(code);
    loop {
        let mut changed = false;
        for (_, statement) in &statements {
            let Some((left, right)) = statement.split_once('=') else {
                continue;
            };
            let tokens = identifier_tokens(left);
            let Some(type_index) = tokens.iter().position(|token| token == "type")
            else {
                continue;
            };
            let Some(alias) = tokens.get(type_index + 1) else {
                continue;
            };
            if contains_any_identifier(right, identifiers) {
                changed |= identifiers.insert(alias.clone());
            }
        }
        if !changed {
            break;
        }
    }
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
