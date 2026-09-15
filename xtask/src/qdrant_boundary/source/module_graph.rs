use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use super::cross_file::BridgeSource;
use super::lexer::code_only;

type ModulePath = Vec<String>;

const MAX_MODULE_DEPTH: usize = 64;
const MAX_SEMANTIC_IDENTITIES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathModule {
    target: String,
    name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ModuleEdges {
    includes: BTreeMap<String, BTreeSet<String>>,
    paths: BTreeMap<String, Vec<PathModule>>,
}

pub(super) fn semantic_modules(
    sources: &[BridgeSource],
    bridge_root: &str,
) -> Option<BTreeMap<String, BTreeSet<ModulePath>>> {
    let source_paths = sources
        .iter()
        .map(|source| source.relative().to_owned())
        .collect::<BTreeSet<_>>();
    let edges = collect_module_edges(sources, &source_paths)?;
    if !include_graph_is_acyclic(&edges.includes) {
        return None;
    }
    let modules = seed_canonical_modules(sources, bridge_root)?;
    propagate_semantic_modules(sources, &edges, modules)
}

fn collect_module_edges(
    sources: &[BridgeSource],
    source_paths: &BTreeSet<String>,
) -> Option<ModuleEdges> {
    let mut edges = ModuleEdges::default();
    for source in sources {
        let relative = source.relative().to_owned();
        for declared in include_paths(source.source())? {
            let target = resolve_target(
                source.relative(),
                &declared,
                source_paths,
            )?;
            edges
                .includes
                .entry(relative.clone())
                .or_default()
                .insert(target);
        }
        for path_module in path_modules(source.source())? {
            let target = resolve_target(
                source.relative(),
                &path_module.target,
                source_paths,
            )?;
            edges
                .paths
                .entry(relative.clone())
                .or_default()
                .push(PathModule {
                    target,
                    name: path_module.name,
                });
        }
    }
    Some(edges)
}

fn seed_canonical_modules(
    sources: &[BridgeSource],
    bridge_root: &str,
) -> Option<BTreeMap<String, BTreeSet<ModulePath>>> {
    let mut modules = BTreeMap::<String, BTreeSet<ModulePath>>::new();
    let mut identity_count = 0_usize;
    for source in sources {
        let module = canonical_module(source.relative(), bridge_root)?;
        if module.len() > MAX_MODULE_DEPTH {
            return None;
        }
        let inserted = modules
            .entry(source.relative().to_owned())
            .or_default()
            .insert(module);
        record_identity(inserted, &mut identity_count)?;
    }
    Some(modules)
}

fn propagate_semantic_modules(
    sources: &[BridgeSource],
    edges: &ModuleEdges,
    mut modules: BTreeMap<String, BTreeSet<ModulePath>>,
) -> Option<BTreeMap<String, BTreeSet<ModulePath>>> {
    let mut identity_count = modules.values().map(BTreeSet::len).sum::<usize>();
    for _ in 0..=sources.len() {
        let mut changed = false;
        for source in sources {
            let Some(parents) = modules.get(source.relative()).cloned() else {
                continue;
            };
            changed |= propagate_includes(
                source.relative(),
                &parents,
                &edges.includes,
                &mut modules,
                &mut identity_count,
            )?;
            changed |= propagate_paths(
                source.relative(),
                &parents,
                &edges.paths,
                &mut modules,
                &mut identity_count,
            )?;
        }
        if !changed {
            return Some(modules);
        }
    }
    None
}

fn propagate_includes(
    source: &str,
    parents: &BTreeSet<ModulePath>,
    edges: &BTreeMap<String, BTreeSet<String>>,
    modules: &mut BTreeMap<String, BTreeSet<ModulePath>>,
    identity_count: &mut usize,
) -> Option<bool> {
    let mut changed = false;
    for target in edges.get(source).into_iter().flatten() {
        for parent in parents {
            let inserted = modules
                .entry(target.clone())
                .or_default()
                .insert(parent.clone());
            changed |= inserted;
            record_identity(inserted, identity_count)?;
        }
    }
    Some(changed)
}

fn propagate_paths(
    source: &str,
    parents: &BTreeSet<ModulePath>,
    edges: &BTreeMap<String, Vec<PathModule>>,
    modules: &mut BTreeMap<String, BTreeSet<ModulePath>>,
    identity_count: &mut usize,
) -> Option<bool> {
    let mut changed = false;
    for path_module in edges.get(source).into_iter().flatten() {
        for parent in parents {
            if parent.len() >= MAX_MODULE_DEPTH {
                return None;
            }
            let mut child = parent.clone();
            child.push(path_module.name.clone());
            let inserted = modules
                .entry(path_module.target.clone())
                .or_default()
                .insert(child);
            changed |= inserted;
            record_identity(inserted, identity_count)?;
        }
    }
    Some(changed)
}

fn record_identity(inserted: bool, count: &mut usize) -> Option<()> {
    if inserted {
        *count = count.checked_add(1)?;
        if *count > MAX_SEMANTIC_IDENTITIES {
            return None;
        }
    }
    Some(())
}

fn include_graph_is_acyclic(
    graph: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let mut indegree = BTreeMap::<String, usize>::new();
    for (source, targets) in graph {
        indegree.entry(source.clone()).or_default();
        for target in targets {
            *indegree.entry(target.clone()).or_default() += 1;
        }
    }
    let mut ready = BTreeSet::new();
    for (path, degree) in &indegree {
        if *degree == 0 {
            ready.insert(path.clone());
        }
    }
    let mut visited = 0_usize;
    while let Some(source) = ready.pop_first() {
        visited = visited.saturating_add(1);
        if let Some(targets) = graph.get(&source) {
            for target in targets {
                let Some(degree) = indegree.get_mut(target) else {
                    return false;
                };
                let Some(next) = degree.checked_sub(1) else {
                    return false;
                };
                *degree = next;
                if next == 0 {
                    ready.insert(target.clone());
                }
            }
        }
    }
    visited == indegree.len()
}

fn canonical_module(relative: &str, bridge_root: &str) -> Option<ModulePath> {
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

fn resolve_target(
    source_relative: &str,
    declared: &str,
    source_paths: &BTreeSet<String>,
) -> Option<String> {
    if declared.is_empty() || declared.contains('\\') {
        return None;
    }
    let parent = Path::new(source_relative).parent()?;
    let joined = parent.join(declared);
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    let relative = normalized
        .iter()
        .map(|component| component.to_str())
        .collect::<Option<Vec<_>>>()?
        .join("/");
    source_paths.contains(&relative).then_some(relative)
}

fn include_paths(source: &str) -> Option<Vec<String>> {
    let code = code_only(source);
    let bytes = source.as_bytes();
    let mut paths = Vec::new();
    let mut cursor = 0_usize;
    while let Some(offset) = code[cursor..].find("include") {
        let start = cursor + offset;
        cursor = start + "include".len();
        if !identifier_boundaries(&code, start, "include".len()) {
            continue;
        }
        let mut index = skip_whitespace(code.as_bytes(), cursor);
        if code.as_bytes().get(index) != Some(&b'!') {
            continue;
        }
        index = skip_whitespace(code.as_bytes(), index + 1);
        if code.as_bytes().get(index) != Some(&b'(') {
            return None;
        }
        let (path, after_literal) = direct_string_literal(source, index + 1)?;
        let close = skip_trivia(bytes, after_literal)?;
        if bytes.get(close) != Some(&b')') {
            return None;
        }
        paths.push(path);
        cursor = close + 1;
    }
    Some(paths)
}

fn path_modules(source: &str) -> Option<Vec<PathModule>> {
    let code = code_only(source);
    let bytes = source.as_bytes();
    let mut modules = Vec::new();
    let mut cursor = 0_usize;
    while let Some(offset) = code[cursor..].find("#[") {
        let start = cursor + offset;
        let Some(close_offset) = code[start..].find(']') else {
            return None;
        };
        let close = start + close_offset + 1;
        cursor = close;
        let compact = code[start..close]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if !compact.starts_with("#[path=") {
            continue;
        }
        let equal = code[start..close].find('=')? + start;
        let (target, after_literal) = direct_string_literal(source, equal + 1)?;
        let attribute_end = skip_trivia(bytes, after_literal)?;
        if bytes.get(attribute_end) != Some(&b']') {
            return None;
        }
        let Some(semicolon_offset) = code[close..].find(';') else {
            return None;
        };
        let statement_end = close + semicolon_offset + 1;
        let statement = &code[close..statement_end];
        if statement.contains('{') {
            return None;
        }
        let tokens = identifiers(statement);
        let mod_index = tokens.iter().position(|token| token == "mod")?;
        let name = tokens.get(mod_index + 1)?.clone();
        modules.push(PathModule { target, name });
        cursor = statement_end;
    }
    Some(modules)
}

fn direct_string_literal(
    text: &str,
    start: usize,
) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let index = skip_trivia(bytes, start)?;
    if let Some((content_start, hashes)) = raw_string_start(bytes, index) {
        let mut end = content_start;
        while end < bytes.len() {
            if bytes[end] == b'"'
                && (0..hashes).all(|offset| {
                    bytes.get(end + 1 + offset) == Some(&b'#')
                })
            {
                let value = String::from_utf8(bytes[content_start..end].to_vec()).ok()?;
                return Some((value, end + 1 + hashes));
            }
            end += 1;
        }
        return None;
    }
    if bytes.get(index) != Some(&b'"') {
        return None;
    }

    let mut output = Vec::new();
    let mut cursor = index + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => {
                return Some((String::from_utf8(output).ok()?, cursor + 1));
            }
            b'\\' => {
                cursor += 1;
                let escaped = *bytes.get(cursor)?;
                match escaped {
                    b'\\' | b'"' => output.push(escaped),
                    _ => return None,
                }
            }
            byte => output.push(byte),
        }
        cursor += 1;
    }
    None
}

fn skip_trivia(bytes: &[u8], mut index: usize) -> Option<usize> {
    loop {
        index = skip_whitespace(bytes, index);
        if bytes.get(index..)?.starts_with(b"//") {
            index += 2;
            while let Some(byte) = bytes.get(index) {
                index += 1;
                if *byte == b'\n' {
                    break;
                }
            }
            continue;
        }
        if bytes.get(index..)?.starts_with(b"/*") {
            index += 2;
            let mut depth = 1_usize;
            while depth > 0 {
                if bytes.get(index..)?.starts_with(b"/*") {
                    depth = depth.checked_add(1)?;
                    index += 2;
                } else if bytes.get(index..)?.starts_with(b"*/") {
                    depth = depth.checked_sub(1)?;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        return Some(index);
    }
}

fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let mut cursor = index;
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let mut hashes = 0_usize;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'"')).then_some((cursor + 1, hashes))
}

fn skip_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn identifier_boundaries(text: &str, start: usize, length: usize) -> bool {
    let before = text[..start].bytes().next_back();
    let after = text[start + length..].bytes().next();
    before.is_none_or(|byte| !identifier_byte(byte))
        && after.is_none_or(|byte| !identifier_byte(byte))
}

fn identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn identifiers(text: &str) -> Vec<String> {
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
mod tests {
    use super::super::cross_file::BridgeSource;
    use super::{MAX_MODULE_DEPTH, semantic_modules};

    const BRIDGE: &str = "crates/search-index-qdrant/search-qdrant-bridge";

    fn source(path: &str, text: &str) -> BridgeSource {
        BridgeSource::new(format!("{BRIDGE}/src/{path}"), text.to_owned())
    }

    fn module(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn literal_include_inherits_the_including_module() {
        let sources = [
            source("real.rs", "include!(\"real/hidden.rs\");\n"),
            source("real/hidden.rs", "type Hidden = ();\n"),
        ];
        let modules = semantic_modules(&sources, BRIDGE).expect("module graph");
        let hidden = modules
            .get(&format!("{BRIDGE}/src/real/hidden.rs"))
            .expect("hidden source");
        assert!(hidden.contains(&module(&["real"])));
        assert!(hidden.contains(&module(&["real", "hidden"])));
    }

    #[test]
    fn path_attribute_uses_the_declared_module_name() {
        let sources = [
            source("lib.rs", "#[path = \"hidden.rs\"]\nmod private;\n"),
            source("hidden.rs", "type Hidden = ();\n"),
        ];
        let modules = semantic_modules(&sources, BRIDGE).expect("module graph");
        let hidden = modules
            .get(&format!("{BRIDGE}/src/hidden.rs"))
            .expect("hidden source");
        assert!(hidden.contains(&module(&["private"])));
        assert!(hidden.contains(&module(&["hidden"])));
    }

    #[test]
    fn inert_directives_do_not_change_the_module_graph() {
        let sources = [
            source(
                "lib.rs",
                r##"// include!("hidden.rs");
const NOTE: &str = "#[path = \"hidden.rs\"] mod private;";
"##,
            ),
            source("hidden.rs", "type Hidden = ();\n"),
        ];
        let modules = semantic_modules(&sources, BRIDGE).expect("module graph");
        let hidden = modules
            .get(&format!("{BRIDGE}/src/hidden.rs"))
            .expect("hidden source");
        assert_eq!(hidden.len(), 1);
        assert!(hidden.contains(&module(&["hidden"])));
    }

    #[test]
    fn literal_include_cycle_fails_closed() {
        let sources = [
            source("a.rs", "include!(\"b.rs\");\n"),
            source("b.rs", "include!(\"a.rs\");\n"),
        ];
        assert!(semantic_modules(&sources, BRIDGE).is_none());
    }

    #[test]
    fn unresolved_or_computed_module_target_fails_closed() {
        for declaration in [
            "include!(\"missing.rs\");\n",
            "include!(concat!(\"missing\", \".rs\"));\n",
            "#[path = \"missing.rs\"]\nmod missing;\n",
        ] {
            let sources = [source("lib.rs", declaration)];
            assert!(
                semantic_modules(&sources, BRIDGE).is_none(),
                "{declaration}"
            );
        }
    }

    #[test]
    fn excessive_canonical_module_depth_fails_closed() {
        let nested = (0..=MAX_MODULE_DEPTH)
            .map(|index| format!("m{index}"))
            .collect::<Vec<_>>()
            .join("/");
        let path = format!("{nested}/leaf.rs");
        let sources = [source(&path, "type Hidden = ();\n")];
        assert!(semantic_modules(&sources, BRIDGE).is_none());
    }

    #[test]
    fn recursive_path_override_fails_closed() {
        let sources = [source(
            "lib.rs",
            "#[path = \"lib.rs\"]\nmod recursive;\n",
        )];
        assert!(semantic_modules(&sources, BRIDGE).is_none());
    }
}
