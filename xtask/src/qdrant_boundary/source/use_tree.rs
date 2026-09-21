//! Shared import/alias owner for both local surfaces and cross-file propagation.

use std::collections::{BTreeMap, BTreeSet};

use super::statements::{
    Statement, StatementKind, collect_statements, identifier_name,
};
use super::tokens::code_tokens;

mod bindings;
mod parse;
use bindings::expand_bindings;
use parse::{ExpansionBudget, expand_use_tree};

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

pub(super) fn direct_tainted_bindings(code: &str, vendor_module: &str) -> DirectTaint {
    let mut direct = DirectTaint::default();
    let declarations = match collect_statements(code) {
        Ok(declarations) => declarations,
        Err(line) => {
            direct.wildcard_lines.insert(line);
            return direct;
        }
    };
    let imports = use_declarations(&declarations);
    for import in &imports {
        if import.leaves.is_none() {
            direct.wildcard_lines.insert(import.line);
        }
    }
    let mut identifiers = BTreeSet::from([vendor_module.to_owned()]);
    for declaration in &declarations {
        if declaration.kind != StatementKind::ExternCrate {
            continue;
        }
        if let Some((name, binding)) = extern_binding(declaration)
            && name == vendor_module && binding != "_"
        {
            identifiers.insert(binding.to_owned());
            merge_binding(&mut direct.bindings, binding.to_owned(), declaration.public);
        }
    }
    expand_bindings(&declarations, &imports, &mut identifiers, &mut direct);
    direct
}

fn extern_binding<'a>(declaration: &Statement<'a>) -> Option<(&'a str, &'a str)> {
    if !declaration.complete {
        return None;
    }
    let mut tokens = code_tokens(declaration.body);
    let name = identifier_name(tokens.next()?)?;
    let binding = match tokens.next() {
        None => name,
        Some("as") => identifier_name(tokens.next()?)?,
        _ => return None,
    };
    tokens.next().is_none().then_some((name, binding))
}

fn merge_binding(bindings: &mut BTreeMap<String, bool>, name: String, exportable: bool) {
    bindings.entry(name)
        .and_modify(|current| *current |= exportable)
        .or_insert(exportable);
}

pub(super) fn collect_use_statements(code: &str) -> Vec<UseStatement> {
    match collect_statements(code) {
        Ok(declarations) => use_declarations(&declarations),
        Err(line) => vec![invalid_statement(line)],
    }
}

fn invalid_statement(line: usize) -> UseStatement {
    UseStatement { line, public: false, leaves: None }
}

fn use_declarations(declarations: &[Statement<'_>]) -> Vec<UseStatement> {
    let mut imports = Vec::new();
    let mut budget = ExpansionBudget::default();
    for declaration in declarations {
        if !declaration.complete {
            imports.push(invalid_statement(declaration.line));
        } else if declaration.kind == StatementKind::Use {
            imports.push(UseStatement {
                line: declaration.line,
                public: declaration.public,
                leaves: expand_use_tree(declaration.body, &mut budget),
            });
        }
    }
    imports
}

pub(super) fn expand_local_aliases(
    code: &str,
    identifiers: &mut BTreeSet<String>,
) -> BTreeSet<usize> {
    // The caller also consumes collect_use_statements, which reports budget and
    // incomplete-declaration failures. Do not manufacture a partial alias set.
    let declarations = match collect_statements(code) {
        Ok(declarations) => declarations,
        Err(line) => return BTreeSet::from([line]),
    };
    let imports = use_declarations(&declarations);
    let mut direct = DirectTaint::default();
    expand_bindings(&declarations, &imports, identifiers, &mut direct);
    direct.wildcard_lines
}
