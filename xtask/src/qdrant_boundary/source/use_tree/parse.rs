//! Finite use-tree expansion; all statements in one file share the same budget.

use std::iter::Peekable;

use super::super::statements::identifier_name;
use super::super::tokens::code_tokens;
use super::UseLeaf;

const MAX_DEPTH: usize = 64;
const MAX_PATH_SEGMENTS: usize = 256;
const MAX_LEAVES: usize = 65_536;
const MAX_EXPANDED_BYTES: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(super) struct ExpansionBudget {
    leaves: usize,
    bytes: usize,
}

impl ExpansionBudget {
    fn push(
        &mut self,
        leaves: &mut Vec<UseLeaf>,
        path: &[String],
        binding: &str,
        glob: bool,
    ) -> Option<()> {
        let bytes = path.iter().try_fold(binding.len(), |sum, part| {
            sum.checked_add(part.len())?.checked_add(size_of::<String>())
        })?.checked_add(size_of::<UseLeaf>())?;
        let total = self.bytes.checked_add(bytes)?;
        if self.leaves >= MAX_LEAVES || total > MAX_EXPANDED_BYTES {
            return None;
        }
        self.leaves += 1;
        self.bytes = total;
        // Charge duplicated prefix/binding storage before allocating a leaf.
        leaves.push(UseLeaf {
            path: path.to_vec(),
            binding: binding.to_owned(),
            glob,
        });
        Some(())
    }
}

pub(super) fn expand_use_tree(
    tree: &str,
    budget: &mut ExpansionBudget,
) -> Option<Vec<UseLeaf>> {
    let mut tokens = code_tokens(tree).peekable();
    let mut leaves = Vec::new();
    fragment(&mut tokens, &mut Vec::new(), 0, budget, &mut leaves)?;
    tokens.next().is_none().then_some(leaves)
}

fn fragment<'a>(
    tokens: &mut Peekable<impl Iterator<Item = &'a str>>,
    path: &mut Vec<String>,
    depth: usize,
    budget: &mut ExpansionBudget,
    leaves: &mut Vec<UseLeaf>,
) -> Option<()> {
    if depth > MAX_DEPTH {
        return None;
    }
    let prefix_len = path.len();
    if tokens.peek() == Some(&":") {
        // A nested absolute root must not be silently prefixed with its parent.
        if !path.is_empty() {
            return None;
        }
        separator(tokens)?;
    }
    loop {
        match tokens.peek().copied()? {
            "{" => {
                let _ = tokens.next();
                loop {
                    if tokens.peek() == Some(&"}") {
                        let _ = tokens.next();
                        break;
                    }
                    fragment(tokens, path, depth + 1, budget, leaves)?;
                    match tokens.next()? {
                        "}" => break,
                        "," => {}
                        _ => return None,
                    }
                }
                break;
            }
            "*" => {
                let _ = tokens.next();
                budget.push(leaves, path, "", true)?;
                break;
            }
            "as" | "_" => return None,
            _ => {
                if path.len() >= MAX_PATH_SEGMENTS {
                    return None;
                }
                path.push(identifier_name(tokens.next()?)?.to_owned());
                if tokens.peek() == Some(&":") {
                    separator(tokens)?;
                    continue;
                }
                // `P::self` and `P::{self}` bind P, not a fictitious self item.
                if path.last().is_some_and(|name| name == "self") {
                    path.pop();
                }
                let binding = if tokens.peek() == Some(&"as") {
                    let _ = tokens.next();
                    identifier_name(tokens.next()?)?
                } else {
                    path.last()?.as_str()
                };
                // An underscore import does not introduce a usable type name.
                if binding != "_" {
                    budget.push(leaves, path, binding, false)?;
                }
                break;
            }
        }
    }
    path.truncate(prefix_len);
    Some(())
}

fn separator<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Option<()> {
    (tokens.next() == Some(":") && tokens.next() == Some(":"))
        .then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_byte_budget_is_charged_before_leaf_allocation() {
        let mut budget = ExpansionBudget { leaves: 0, bytes: MAX_EXPANDED_BYTES - 1 };
        assert!(expand_use_tree("owned::Value", &mut budget).is_none());
        assert_eq!(budget.leaves, 0);
        assert_eq!(budget.bytes, MAX_EXPANDED_BYTES - 1);
    }

    #[test]
    fn leaf_budget_is_shared_by_separate_import_statements() {
        let mut budget = ExpansionBudget { leaves: MAX_LEAVES - 1, bytes: 0 };
        assert_eq!(expand_use_tree("owned::A", &mut budget).expect("last leaf").len(), 1);
        let charged = budget.bytes;
        assert!(expand_use_tree("owned::B", &mut budget).is_none());
        assert_eq!(budget.leaves, MAX_LEAVES);
        assert_eq!(budget.bytes, charged);
    }

    #[test]
    fn malformed_suffix_does_not_refund_expanded_prefix_work() {
        let mut budget = ExpansionBudget::default();
        assert!(expand_use_tree("owned::{A, B,,}", &mut budget).is_none());
        assert_eq!(budget.leaves, 2);
        assert!(budget.bytes > 0);
    }
}
