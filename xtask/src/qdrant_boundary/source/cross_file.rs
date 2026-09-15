use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::lexer::code_only;
use super::module_graph::semantic_modules;
use super::surface::find_public_vendor_surfaces;

mod use_tree;

use use_tree::{
    DirectTaint, UseLeaf, collect_use_statements, direct_tainted_bindings,
    expand_local_aliases,
};

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

    #[must_use]
    pub(super) fn relative(&self) -> &str {
        &self.relative
    }

    #[must_use]
    pub(super) fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Clone, Debug)]
struct SourceUnit {
    relative: Arc<str>,
    module: Vec<String>,
    code: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedItem {
    module: Vec<String>,
    name: String,
}

#[derive(Clone, Debug, Default)]
struct TaintInfo {
    all: BTreeSet<Arc<str>>,
    exportable: BTreeSet<Arc<str>>,
}

type TaintedItems = BTreeMap<Vec<String>, BTreeMap<String, TaintInfo>>;

/// Finds vendor-tainted aliases imported or publicly re-exported across bridge
/// source files.
///
/// The file-local scanner owns direct SDK references. This pass closes the
/// standard Rust module-path escape where a private alias is defined in one
/// file, then imported into a public signature or re-exported by another file.
/// Literal `include!`, direct `#[path]` module overrides and glob imports are
/// resolved inside the bounded bridge source inventory.
pub(super) fn find_cross_file_vendor_surfaces(
    sources: &[BridgeSource],
    bridge_root: &str,
    vendor_module: &str,
) -> Vec<(String, usize)> {
    let Some(units) = source_units(sources, bridge_root) else {
        return fail_closed_source_findings(sources);
    };
    let mut findings = BTreeSet::new();
    let mut tainted_items =
        seed_tainted_items(&units, vendor_module, &mut findings);
    propagate_public_reexports(
        &units,
        &mut tainted_items,
        &mut findings,
    );
    collect_public_surface_findings(
        &units,
        &tainted_items,
        &mut findings,
    );
    findings.into_iter().collect()
}

fn fail_closed_source_findings(
    sources: &[BridgeSource],
) -> Vec<(String, usize)> {
    let mut findings = sources
        .iter()
        .map(|source| (source.relative().to_owned(), 1))
        .collect::<Vec<_>>();
    findings.sort();
    findings.dedup();
    findings
}

fn source_units(
    sources: &[BridgeSource],
    bridge_root: &str,
) -> Option<Vec<SourceUnit>> {
    let semantic = semantic_modules(sources, bridge_root)?;
    let mut units = Vec::new();
    for source in sources {
        let Some(modules) = semantic.get(source.relative()) else {
            continue;
        };
        let relative = Arc::<str>::from(source.relative());
        let code = Arc::<str>::from(code_only(source.source()));
        for module in modules {
            units.push(SourceUnit {
                relative: Arc::clone(&relative),
                module: module.clone(),
                code: Arc::clone(&code),
            });
        }
    }
    Some(units)
}

fn seed_tainted_items(
    units: &[SourceUnit],
    vendor_module: &str,
    findings: &mut BTreeSet<(String, usize)>,
) -> TaintedItems {
    let mut tainted_items = TaintedItems::new();
    let mut direct_by_source = BTreeMap::<Arc<str>, DirectTaint>::new();
    for unit in units {
        let direct = direct_by_source
            .entry(Arc::clone(&unit.relative))
            .or_insert_with(|| {
                direct_tainted_bindings(&unit.code, vendor_module)
            });
        for line in &direct.wildcard_lines {
            findings.insert((unit.relative.to_string(), *line));
        }
        for (binding, exportable) in &direct.bindings {
            insert_taint(
                &mut tainted_items,
                &unit.module,
                binding.clone(),
                &unit.relative,
                *exportable,
            );
        }
    }
    tainted_items
}

fn propagate_public_reexports(
    units: &[SourceUnit],
    tainted_items: &mut TaintedItems,
    findings: &mut BTreeSet<(String, usize)>,
) {
    loop {
        let mut changed = false;
        for unit in units {
            for statement in collect_use_statements(&unit.code) {
                if !statement.public {
                    continue;
                }
                let Some(leaves) = &statement.leaves else {
                    findings.insert((unit.relative.to_string(), statement.line));
                    continue;
                };
                for leaf in leaves {
                    if leaf.glob {
                        changed |= propagate_public_glob(
                            unit,
                            statement.line,
                            leaf,
                            tainted_items,
                            findings,
                        );
                    } else {
                        changed |= propagate_public_item(
                            unit,
                            statement.line,
                            leaf,
                            tainted_items,
                            findings,
                        );
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn propagate_public_glob(
    unit: &SourceUnit,
    line: usize,
    leaf: &UseLeaf,
    tainted_items: &mut TaintedItems,
    findings: &mut BTreeSet<(String, usize)>,
) -> bool {
    let Some(source_module) = resolve_module(&unit.module, &leaf.path) else {
        return false;
    };
    if source_module == unit.module {
        return false;
    }
    let names = tainted_names(tainted_items, &source_module, None, true);
    if names.is_empty() {
        return false;
    }
    findings.insert((unit.relative.to_string(), line));
    let mut changed = false;
    for name in names {
        changed |= insert_taint(
            tainted_items,
            &unit.module,
            name,
            &unit.relative,
            true,
        );
    }
    changed
}

fn propagate_public_item(
    unit: &SourceUnit,
    line: usize,
    leaf: &UseLeaf,
    tainted_items: &mut TaintedItems,
    findings: &mut BTreeSet<(String, usize)>,
) -> bool {
    let Some(resolved) = resolve_item(&unit.module, &leaf.path) else {
        return false;
    };
    if resolved.module == unit.module
        || !is_tainted(tainted_items, &resolved.module, &resolved.name)
    {
        return false;
    }
    findings.insert((unit.relative.to_string(), line));
    insert_taint(
        tainted_items,
        &unit.module,
        leaf.binding.clone(),
        &unit.relative,
        true,
    )
}

fn collect_public_surface_findings(
    units: &[SourceUnit],
    tainted_items: &TaintedItems,
    findings: &mut BTreeSet<(String, usize)>,
) {
    for unit in units {
        let mut visible_tainted_names = tainted_names(
            tainted_items,
            &unit.module,
            Some(unit.relative.as_ref()),
            false,
        );
        add_imported_taint(
            unit,
            tainted_items,
            &mut visible_tainted_names,
            findings,
        );
        expand_local_aliases(&unit.code, &mut visible_tainted_names);
        if visible_tainted_names.is_empty() {
            continue;
        }
        for line in
            find_public_vendor_surfaces(&unit.code, &visible_tainted_names)
        {
            findings.insert((unit.relative.to_string(), line));
        }
    }
}

fn add_imported_taint(
    unit: &SourceUnit,
    tainted_items: &TaintedItems,
    visible: &mut BTreeSet<String>,
    findings: &mut BTreeSet<(String, usize)>,
) {
    for statement in collect_use_statements(&unit.code) {
        let Some(leaves) = &statement.leaves else {
            findings.insert((unit.relative.to_string(), statement.line));
            continue;
        };
        for leaf in leaves {
            if leaf.glob {
                let Some(source_module) =
                    resolve_module(&unit.module, &leaf.path)
                else {
                    continue;
                };
                if source_module != unit.module {
                    visible.extend(tainted_names(
                        tainted_items,
                        &source_module,
                        None,
                        statement.public,
                    ));
                }
                continue;
            }

            let Some(resolved) = resolve_item(&unit.module, &leaf.path) else {
                continue;
            };
            if resolved.module != unit.module
                && is_tainted(
                    tainted_items,
                    &resolved.module,
                    &resolved.name,
                )
            {
                visible.insert(resolved.name);
                visible.insert(leaf.binding);
            }
        }
    }
}

fn resolve_item(current_module: &[String], path: &[String]) -> Option<ResolvedItem> {
    let (name, prefix) = path.split_last()?;
    if name == "self" || name == "super" || name == "crate" {
        return None;
    }
    Some(ResolvedItem {
        module: resolve_module(current_module, prefix)?,
        name: name.clone(),
    })
}

fn resolve_module(current_module: &[String], path: &[String]) -> Option<Vec<String>> {
    let mut module = Vec::new();
    let mut index = 0_usize;
    match path.first().map(String::as_str) {
        Some("crate") => index = 1,
        Some("self") => {
            module.extend_from_slice(current_module);
            index = 1;
        }
        Some("super") => {
            module.extend_from_slice(current_module);
            while path.get(index).map(String::as_str) == Some("super") {
                module.pop()?;
                index += 1;
            }
        }
        _ => {}
    }
    module.extend(path[index..].iter().cloned());
    Some(module)
}

fn insert_taint(
    tainted: &mut TaintedItems,
    module: &[String],
    name: String,
    origin: &Arc<str>,
    exportable: bool,
) -> bool {
    let info = tainted
        .entry(module.to_vec())
        .or_default()
        .entry(name)
        .or_default();
    let mut changed = info.all.insert(Arc::clone(origin));
    if exportable {
        changed |= info.exportable.insert(Arc::clone(origin));
    }
    changed
}

fn is_tainted(tainted: &TaintedItems, module: &[String], name: &str) -> bool {
    tainted
        .get(module)
        .is_some_and(|names| names.contains_key(name))
}

fn tainted_names(
    tainted: &TaintedItems,
    module: &[String],
    exclude_origin: Option<&str>,
    exportable_only: bool,
) -> BTreeSet<String> {
    let Some(names) = tainted.get(module) else {
        return BTreeSet::new();
    };
    names
        .iter()
        .filter(|(_, info)| {
            let origins = if exportable_only {
                &info.exportable
            } else {
                &info.all
            };
            !origins.is_empty()
                && exclude_origin.is_none_or(|excluded| {
                    origins.iter().any(|origin| origin.as_ref() != excluded)
                })
        })
        .map(|(name, _)| name.clone())
        .collect()
}

#[cfg(test)]
mod tests;
