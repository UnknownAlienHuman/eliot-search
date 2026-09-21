//! Work-list propagation: cycles and reversed alias chains do not rescan a file.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::super::imports::reference_name;
use super::super::statements::{Statement, type_alias_parts};
use super::super::tokens::code_tokens;
use super::{DirectTaint, UseStatement, merge_binding};

const MAX_ALIAS_EDGES: usize = 262_144;
type Edges<'a> = BTreeMap<&'a str, BTreeMap<&'a str, bool>>;

pub(super) fn expand_bindings(
    declarations: &[Statement<'_>],
    imports: &[UseStatement],
    identifiers: &mut BTreeSet<String>,
    direct: &mut DirectTaint,
) {
    let mut edges = Edges::new();
    let mut edge_count = 0_usize;
    let mut globs = BTreeMap::<&str, BTreeSet<usize>>::new();
    for import in imports {
        let Some(leaves) = &import.leaves else { continue };
        for leaf in leaves {
            let root = if leaf.path.first().is_some_and(|name| name == "self") {
                leaf.path.get(1)
            } else {
                leaf.path.first()
            };
            let Some(root) = root else { continue };
            if leaf.glob {
                globs.entry(root).or_default().insert(import.line);
            } else if !add_edge(
                &mut edges, &mut edge_count, root, &leaf.binding, import.public,
            ) {
                direct.wildcard_lines.insert(import.line);
                return;
            }
        }
    }
    for declaration in declarations {
        let Some((alias, definition)) = type_alias_parts(declaration) else {
            continue;
        };
        for name in code_tokens(definition).filter_map(reference_name) {
            if !add_edge(&mut edges, &mut edge_count, name, alias, declaration.public) {
                direct.wildcard_lines.insert(declaration.line);
                return;
            }
        }
    }

    let mut pending: VecDeque<String> = identifiers.iter().cloned().collect();
    while let Some(name) = pending.pop_front() {
        if let Some(lines) = globs.get(name.as_str()) {
            direct.wildcard_lines.extend(lines.iter().copied());
        }
        if let Some(targets) = edges.get(name.as_str()) {
            for (&target, &public) in targets {
                merge_binding(&mut direct.bindings, target.to_owned(), public);
                if identifiers.insert(target.to_owned()) {
                    pending.push_back(target.to_owned());
                }
            }
        }
    }
}

fn add_edge<'a>(
    edges: &mut Edges<'a>,
    count: &mut usize,
    source: &'a str,
    target: &'a str,
    public: bool,
) -> bool {
    if let Some(existing) = edges.get_mut(source).and_then(|targets| targets.get_mut(target)) {
        *existing |= public;
        return true;
    }
    if *count >= MAX_ALIAS_EDGES {
        return false;
    }
    *count += 1;
    // Keys borrow declaration/import storage; a repeated long alias name is not
    // copied once per reference. Charge the distinct edge before allocating it.
    edges.entry(source).or_default().insert(target, public);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_edges_merge_visibility_without_spending_the_budget_again() {
        let mut edges = Edges::new();
        let mut count = MAX_ALIAS_EDGES - 1;
        assert!(add_edge(&mut edges, &mut count, "A", "B", false));
        assert!(add_edge(&mut edges, &mut count, "A", "B", true));
        assert_eq!(count, MAX_ALIAS_EDGES);
        assert!(edges["A"]["B"]);
        assert!(!add_edge(&mut edges, &mut count, "B", "C", true));
        assert!(!edges.contains_key("B"));
    }
}
