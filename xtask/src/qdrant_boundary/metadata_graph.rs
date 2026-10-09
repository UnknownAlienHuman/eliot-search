//! Dependency boundary over Cargo's resolved graph, never a second resolver.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{self, Write as _};

use crate::cargo_metadata_adapter::{
    CargoDependency, CargoInventory, CargoNode, CargoPackage, DependencyKind,
};

use super::BRIDGE_MANIFEST;

const VENDOR: &str = "qdrant-client";
const BRIDGE: &str = "search-qdrant-bridge";
const REGISTRY_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
const MAX_DIAGNOSTICS: usize = 128;
const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
const MAX_LABEL_BYTES: usize = 512;
const LABEL_TRUNCATED: &str = "...[label truncated]";
const TEXT_TRUNCATED: &str = "\n[diagnostic text truncated]";
const DIAGNOSTICS_TRUNCATED: &str =
    "cargo metadata: diagnostics truncated by count/text/label limits; validation failed";

type Packages<'a> = BTreeMap<&'a str, &'a CargoPackage>;
type Nodes<'a> = BTreeMap<&'a str, &'a CargoNode>;

/// Check complete metadata. A no-deps inventory cannot prove this boundary.
#[must_use]
pub(super) fn validate_graph(inventory: &CargoInventory, expected_version: &str) -> Vec<String> {
    let mut errors = Diagnostics::default();
    let Some(graph) = Graph::new(inventory, &mut errors) else {
        return errors.finish();
    };
    if expected_version.is_empty() {
        errors.push(format_args!(
            "cargo metadata: expected qdrant-client version is empty"
        ));
    }
    let bridge = graph.workspace_bridge(inventory, &mut errors);
    validate_vendor(&graph, expected_version, &mut errors);

    let members = sorted_text(&inventory.workspace_members);
    for &id in &members {
        if let Some(package) = graph.packages.get(id)
            && bridge.is_none_or(|bridge| bridge.id != package.id)
        {
            for dependency in vendor_declarations(package) {
                errors.push(format_args!(
                    "cargo metadata: {} [{}] declares forbidden direct {:?} {VENDOR} dependency \
                     (alias {:?}, optional {}, target {:?})",
                    Label(&package.name),
                    Label(&package.id),
                    dependency.kind,
                    dependency.rename.as_deref().map(Label),
                    dependency.optional,
                    dependency.target.as_deref().map(Label),
                ));
            }
        }
    }
    if let Some(bridge) = bridge {
        validate_bridge(bridge, &graph, expected_version, &mut errors);
    }
    let bridge_id = bridge.map(|package| package.id.as_str());
    for &id in &members {
        if errors.is_full() {
            errors.truncated = true;
            break;
        }
        if let Some(path) = graph.shortest_forbidden_path(id, bridge_id) {
            errors.push(format_args!(
                "cargo metadata: forbidden production dependency path: {}",
                DependencyPath(&path),
            ));
        }
    }
    errors.finish()
}

#[derive(Default)]
struct Diagnostics {
    entries: Vec<String>,
    truncated: bool,
}

impl Diagnostics {
    const fn is_full(&self) -> bool {
        self.entries.len() == MAX_DIAGNOSTICS
    }

    const fn has_errors(&self) -> bool {
        !self.entries.is_empty() || self.truncated
    }

    // Arguments borrow their labels. No complete diagnostic is allocated
    // before the count check or outside the checked formatting sink.
    fn push(&mut self, arguments: fmt::Arguments<'_>) {
        if self.is_full() {
            self.truncated = true;
            return;
        }
        let mut text = DiagnosticText::new();
        if text.write_fmt(arguments).is_err() {
            text.truncated = true;
            // The writer always reserves these bytes before appending text.
            text.value.push_str(TEXT_TRUNCATED);
        }
        self.truncated |= text.truncated;
        self.entries.push(text.value);
    }

    fn finish(mut self) -> Vec<String> {
        self.entries.sort_unstable();
        self.entries.dedup();
        if self.truncated {
            // The failure marker shares the same count budget. Truncation can
            // never produce an empty, apparently successful result.
            self.entries.truncate(MAX_DIAGNOSTICS - 1);
            self.entries.push(DIAGNOSTICS_TRUNCATED.to_owned());
            self.entries.sort_unstable();
        }
        self.entries
    }
}

struct DiagnosticText {
    value: String,
    truncated: bool,
}

impl DiagnosticText {
    fn new() -> Self {
        Self {
            value: String::with_capacity(MAX_DIAGNOSTIC_BYTES),
            truncated: false,
        }
    }
}

impl fmt::Write for DiagnosticText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let remaining =
            (MAX_DIAGNOSTIC_BYTES - TEXT_TRUNCATED.len()).saturating_sub(self.value.len());
        if value.len() > remaining {
            self.value.push_str(utf8_prefix(value, remaining));
            return Err(fmt::Error);
        }
        self.truncated |= value == LABEL_TRUNCATED;
        self.value.push_str(value);
        Ok(())
    }
}

const fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    let mut end = if value.len() > max_bytes {
        max_bytes
    } else {
        value.len()
    };
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.split_at(end).0
}

#[derive(Clone, Copy)]
struct Label<'a>(&'a str);

impl<'a> Label<'a> {
    const fn prefix(self) -> &'a str {
        if self.0.len() > MAX_LABEL_BYTES {
            utf8_prefix(self.0, MAX_LABEL_BYTES - LABEL_TRUNCATED.len())
        } else {
            self.0
        }
    }
}

impl fmt::Display for Label<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.prefix())?;
        if self.0.len() > MAX_LABEL_BYTES {
            formatter.write_str(LABEL_TRUNCATED)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Label<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.prefix(), formatter)?;
        if self.0.len() > MAX_LABEL_BYTES {
            formatter.write_str(LABEL_TRUNCATED)?;
        }
        Ok(())
    }
}

struct DependencyPath<'a, 'p>(&'a [&'p CargoPackage]);

impl fmt::Display for DependencyPath<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, package) in self.0.iter().enumerate() {
            if position != 0 {
                formatter.write_str(" -> ")?;
            }
            write!(
                formatter,
                "{} [{}]",
                Label(&package.name),
                Label(&package.id)
            )?;
        }
        Ok(())
    }
}

fn sorted_text(values: &[String]) -> Vec<&str> {
    let mut sorted: Vec<_> = values.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    sorted
}

fn vendor_declarations(package: &CargoPackage) -> Vec<&CargoDependency> {
    let mut declarations: Vec<_> = package
        .dependencies
        .iter()
        .filter(|dependency| dependency.name == VENDOR)
        .collect();
    declarations.sort_unstable_by(|left, right| {
        (
            left.kind,
            left.rename.as_deref(),
            left.target.as_deref(),
            left.optional,
        )
            .cmp(&(
                right.kind,
                right.rename.as_deref(),
                right.target.as_deref(),
                right.optional,
            ))
    });
    declarations
}

struct Graph<'a> {
    packages: Packages<'a>,
    nodes: Nodes<'a>,
}

impl<'a> Graph<'a> {
    fn new(inventory: &'a CargoInventory, errors: &mut Diagnostics) -> Option<Self> {
        let Some(resolve) = &inventory.resolve else {
            errors.push(format_args!(
                "cargo metadata: full resolve graph is required"
            ));
            return None;
        };
        let packages = index_packages(&inventory.packages, errors);
        let nodes = index_nodes(resolve, errors);
        // Duplicate identities have no authoritative record to traverse.
        if errors.has_errors() {
            return None;
        }
        validate_members(inventory, &packages, &nodes, errors);
        validate_edges(&packages, &nodes, errors);
        if errors.has_errors() {
            None
        } else {
            Some(Self { packages, nodes })
        }
    }

    fn workspace_bridge(
        &self,
        inventory: &CargoInventory,
        errors: &mut Diagnostics,
    ) -> Option<&'a CargoPackage> {
        // Join constant components separately: a Windows verbatim root keeps
        // forward slashes literal, while Cargo emits native manifest paths.
        let expected_manifest = BRIDGE_MANIFEST
            .split('/')
            .fold(inventory.workspace_root.clone(), |path, part| {
                path.join(part)
            });
        let candidates: Vec<_> = inventory
            .workspace_members
            .iter()
            .filter_map(|id| self.packages.get(id.as_str()).copied())
            .filter(|package| package.manifest_path == expected_manifest)
            .collect();
        let [bridge] = candidates.as_slice() else {
            errors.push(format_args!(
                "cargo metadata: expected one workspace {BRIDGE} at {BRIDGE_MANIFEST}, found {}",
                candidates.len(),
            ));
            return None;
        };
        if bridge.name != BRIDGE {
            errors.push(format_args!(
                "cargo metadata: {BRIDGE_MANIFEST} must name {BRIDGE}, found {:?}",
                Label(&bridge.name),
            ));
            return None;
        }
        if bridge.source.is_some() {
            errors.push(format_args!(
                "cargo metadata: workspace {BRIDGE} must be a local package"
            ));
            return None;
        }
        Some(*bridge)
    }

    // Breadth-first predecessors bound storage to the graph, rather than
    // cloning a complete path for every queued edge. All target predicates
    // participate; only development-only edges are excluded.
    fn shortest_forbidden_path(
        &self,
        start: &'a str,
        bridge_id: Option<&str>,
    ) -> Option<Vec<&'a CargoPackage>> {
        let mut parents = BTreeMap::from([(start, None)]);
        let mut queue = VecDeque::from([start]);
        while let Some(id) = queue.pop_front() {
            if Some(id) == bridge_id {
                continue;
            }
            let package = self.packages.get(id)?;
            if package.name == VENDOR {
                return self.path(id, &parents);
            }
            let node = self.nodes.get(id)?;
            let mut neighbors: Vec<_> = node
                .dependencies
                .iter()
                .filter(|dependency| {
                    dependency.kinds.iter().any(|kind| {
                        matches!(kind.kind, DependencyKind::Normal | DependencyKind::Build)
                    })
                })
                .map(|dependency| dependency.package_id.as_str())
                .collect();
            neighbors.sort_unstable_by_key(|id| {
                self.packages
                    .get(id)
                    .map(|package| (package.name.as_str(), package.id.as_str()))
            });
            neighbors.dedup();
            for neighbor in neighbors {
                if let std::collections::btree_map::Entry::Vacant(entry) = parents.entry(neighbor) {
                    entry.insert(Some(id));
                    queue.push_back(neighbor);
                }
            }
        }
        None
    }

    fn path(
        &self,
        end: &'a str,
        parents: &BTreeMap<&'a str, Option<&'a str>>,
    ) -> Option<Vec<&'a CargoPackage>> {
        let mut path = Vec::new();
        let mut cursor = Some(end);
        while let Some(id) = cursor {
            path.push(*self.packages.get(id)?);
            cursor = *parents.get(id)?;
        }
        path.reverse();
        Some(path)
    }
}

fn index_packages<'a>(packages: &'a [CargoPackage], errors: &mut Diagnostics) -> Packages<'a> {
    let mut indexed = BTreeMap::new();
    let mut packages: Vec<_> = packages.iter().collect();
    packages.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    for package in packages {
        if package.id.is_empty() {
            errors.push(format_args!("cargo metadata: empty package ID"));
        }
        if indexed.insert(package.id.as_str(), package).is_some() {
            errors.push(format_args!(
                "cargo metadata: duplicate package ID {:?}",
                Label(&package.id)
            ));
        }
    }
    indexed
}

fn index_nodes<'a>(nodes: &'a [CargoNode], errors: &mut Diagnostics) -> Nodes<'a> {
    let mut indexed = BTreeMap::new();
    let mut nodes: Vec<_> = nodes.iter().collect();
    nodes.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    for node in nodes {
        if node.id.is_empty() {
            errors.push(format_args!("cargo metadata: empty resolve node ID"));
        }
        if indexed.insert(node.id.as_str(), node).is_some() {
            errors.push(format_args!(
                "cargo metadata: duplicate resolve node ID {:?}",
                Label(&node.id)
            ));
        }
    }
    indexed
}

fn validate_members(
    inventory: &CargoInventory,
    packages: &Packages<'_>,
    nodes: &Nodes<'_>,
    errors: &mut Diagnostics,
) {
    if inventory.workspace_members.is_empty() {
        errors.push(format_args!("cargo metadata: workspace has no members"));
    }
    let mut members = BTreeSet::new();
    for id in sorted_text(&inventory.workspace_members) {
        if !members.insert(id) {
            errors.push(format_args!(
                "cargo metadata: duplicate workspace member {:?}",
                Label(id)
            ));
        }
        if !packages.contains_key(id) || !nodes.contains_key(id) {
            errors.push(format_args!(
                "cargo metadata: workspace member {:?} lacks package/resolve node",
                Label(id),
            ));
        }
    }
    let mut defaults = BTreeSet::new();
    for id in sorted_text(&inventory.workspace_default_members) {
        if !defaults.insert(id) {
            errors.push(format_args!(
                "cargo metadata: duplicate default member {:?}",
                Label(id)
            ));
        }
        if !members.contains(id) {
            errors.push(format_args!(
                "cargo metadata: default member {:?} is not a workspace member",
                Label(id),
            ));
        }
    }
}

fn validate_edges(packages: &Packages<'_>, nodes: &Nodes<'_>, errors: &mut Diagnostics) {
    for node in nodes.values() {
        if !packages.contains_key(node.id.as_str()) {
            errors.push(format_args!(
                "cargo metadata: resolve node {:?} has no package",
                Label(&node.id)
            ));
        }
        let mut dependencies: Vec<_> = node.dependencies.iter().collect();
        dependencies.sort_unstable_by(|left, right| {
            (
                left.package_id.as_str(),
                left.name.as_str(),
                left.kinds.is_empty(),
            )
                .cmp(&(
                    right.package_id.as_str(),
                    right.name.as_str(),
                    right.kinds.is_empty(),
                ))
        });
        for dependency in dependencies {
            if !packages.contains_key(dependency.package_id.as_str())
                || !nodes.contains_key(dependency.package_id.as_str())
            {
                errors.push(format_args!(
                    "cargo metadata: dangling resolve edge {:?} -> {:?}",
                    Label(&node.id),
                    Label(&dependency.package_id),
                ));
            }
            if dependency.kinds.is_empty() {
                errors.push(format_args!(
                    "cargo metadata: resolve edge {:?} -> {:?} has no dependency kind",
                    Label(&node.id),
                    Label(&dependency.package_id),
                ));
            }
        }
    }
}

fn validate_vendor(graph: &Graph<'_>, expected_version: &str, errors: &mut Diagnostics) {
    let vendors: Vec<_> = graph
        .packages
        .values()
        .filter(|package| package.name == VENDOR)
        .collect();
    if vendors.len() != 1 {
        errors.push(format_args!(
            "cargo metadata: expected one resolved {VENDOR} package, found {}",
            vendors.len(),
        ));
    }
    for package in vendors {
        if package.version != expected_version || package.source.as_deref() != Some(REGISTRY_SOURCE)
        {
            let source = package.source.as_deref().unwrap_or("local/path");
            errors.push(format_args!(
                "cargo metadata: {VENDOR} [{}] must be version ={} from \
                 {REGISTRY_SOURCE}; found version {} source {:?}",
                Label(&package.id),
                Label(expected_version),
                Label(&package.version),
                Label(source),
            ));
        }
        if let Some(node) = graph.nodes.get(package.id.as_str()) {
            if !node.features.is_empty() {
                errors.push(format_args!(
                    "cargo metadata: {VENDOR} [{}] must enable no features, including defaults",
                    Label(&package.id),
                ));
            }
        } else {
            errors.push(format_args!(
                "cargo metadata: {VENDOR} [{}] has no resolve node",
                Label(&package.id),
            ));
        }
    }
}

fn validate_bridge(
    bridge: &CargoPackage,
    graph: &Graph<'_>,
    expected_version: &str,
    errors: &mut Diagnostics,
) {
    let declarations = vendor_declarations(bridge);
    if declarations.len() != 1 {
        errors.push(format_args!(
            "cargo metadata: {BRIDGE} must declare exactly one {VENDOR} dependency, found {}",
            declarations.len(),
        ));
    }
    for dependency in declarations {
        if !qualified_declaration(dependency, expected_version) {
            errors.push(format_args!(
                "cargo metadata: {BRIDGE} {:?} {VENDOR} declaration (alias {:?}, target {:?}) \
                 must use exact ={}, normal/unconditional/nonoptional, \
                 the qualified registry and no feature/default/source overrides",
                dependency.kind,
                dependency.rename.as_deref().map(Label),
                dependency.target.as_deref().map(Label),
                Label(expected_version),
            ));
        }
    }
    let normal_vendor = graph.nodes.get(bridge.id.as_str()).is_some_and(|node| {
        node.dependencies.iter().any(|dependency| {
            graph
                .packages
                .get(dependency.package_id.as_str())
                .is_some_and(|package| {
                    package.name == VENDOR
                        && dependency.kinds.iter().any(|kind| {
                            kind.kind == DependencyKind::Normal && kind.target.is_none()
                        })
                })
        })
    });
    if !normal_vendor {
        errors.push(format_args!(
            "cargo metadata: {BRIDGE} lacks a resolved unconditional normal {VENDOR} edge",
        ));
    }
}

fn qualified_declaration(dependency: &CargoDependency, expected_version: &str) -> bool {
    dependency.kind == DependencyKind::Normal
        && !dependency.optional
        && dependency.target.is_none()
        && dependency.requirement.strip_prefix('=') == Some(expected_version)
        && dependency.source.as_deref() == Some(REGISTRY_SOURCE)
        && dependency.registry.is_none()
        && dependency.path.is_none()
        && !dependency.uses_default_features
        && dependency.features.is_empty()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cargo_metadata_adapter::{CargoDependencyKind, CargoResolvedDependency};

    use super::*;

    const VERSION: &str = "1.19.0";
    const SDK_ID: &str = "opaque-sdk-id";

    #[cfg(windows)]
    #[test]
    fn windows_verbatim_root_matches_exact_workspace_bridge_path() {
        let mut inventory = inventory();
        inventory.workspace_root = PathBuf::from(r"\\?\C:\fixture");
        package_mut(&mut inventory, "bridge-id").manifest_path = PathBuf::from(
            r"\\?\C:\fixture\crates\search-index-qdrant\search-qdrant-bridge\Cargo.toml",
        );
        assert!(validate_graph(&inventory, VERSION).is_empty());
    }

    fn package(id: &str, name: &str) -> CargoPackage {
        CargoPackage {
            id: id.to_owned(),
            name: name.to_owned(),
            version: if name == VENDOR { VERSION } else { "0.1.0" }.to_owned(),
            manifest_path: PathBuf::from(format!("fixture/{id}/Cargo.toml")),
            source: (name == VENDOR).then(|| REGISTRY_SOURCE.to_owned()),
            dependencies: Vec::new(),
        }
    }

    fn edge(package_id: &str, kind: DependencyKind) -> CargoResolvedDependency {
        CargoResolvedDependency {
            // Traversal must use the package ID, not this potentially renamed
            // library name or a hyphen/underscore spelling heuristic.
            name: "renamed_library".to_owned(),
            package_id: package_id.to_owned(),
            kinds: vec![CargoDependencyKind { kind, target: None }],
        }
    }

    fn node(id: &str, dependencies: Vec<CargoResolvedDependency>) -> CargoNode {
        CargoNode {
            id: id.to_owned(),
            dependencies,
            features: Vec::new(),
        }
    }

    fn declaration(kind: DependencyKind) -> CargoDependency {
        CargoDependency {
            name: VENDOR.to_owned(),
            rename: None,
            requirement: format!("={VERSION}"),
            kind,
            optional: false,
            target: None,
            source: Some(REGISTRY_SOURCE.to_owned()),
            registry: None,
            path: None,
            uses_default_features: false,
            features: Vec::new(),
        }
    }

    fn inventory() -> CargoInventory {
        let mut bridge = package("bridge-id", BRIDGE);
        bridge.manifest_path = PathBuf::from("fixture").join(BRIDGE_MANIFEST);
        bridge
            .dependencies
            .push(declaration(DependencyKind::Normal));
        CargoInventory {
            workspace_root: PathBuf::from("fixture"),
            workspace_members: vec!["app-id".to_owned(), "bridge-id".to_owned()],
            workspace_default_members: vec!["app-id".to_owned()],
            packages: vec![
                package("app-id", "application"),
                bridge,
                package(SDK_ID, VENDOR),
            ],
            resolve: Some(vec![
                node("app-id", vec![edge("bridge-id", DependencyKind::Normal)]),
                node("bridge-id", vec![edge(SDK_ID, DependencyKind::Normal)]),
                node(SDK_ID, Vec::new()),
            ]),
        }
    }

    fn package_mut<'a>(inventory: &'a mut CargoInventory, id: &str) -> &'a mut CargoPackage {
        inventory
            .packages
            .iter_mut()
            .find(|package| package.id == id)
            .expect("fixture package")
    }

    fn node_mut<'a>(inventory: &'a mut CargoInventory, id: &str) -> &'a mut CargoNode {
        inventory
            .resolve
            .as_mut()
            .expect("fixture resolve")
            .iter_mut()
            .find(|node| node.id == id)
            .expect("fixture node")
    }

    fn external(
        inventory: &mut CargoInventory,
        id: &str,
        name: &str,
        dependencies: Vec<CargoResolvedDependency>,
    ) {
        let mut external = package(id, name);
        external.source = Some(REGISTRY_SOURCE.to_owned());
        inventory.packages.push(external);
        inventory
            .resolve
            .as_mut()
            .expect("fixture resolve")
            .push(node(id, dependencies));
    }

    fn assert_error(inventory: &CargoInventory, expected: &str) {
        let errors = validate_graph(inventory, VERSION);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{expected}: {errors:?}"
        );
    }

    fn reverse_inventory(inventory: &mut CargoInventory) {
        inventory.packages.reverse();
        inventory.workspace_members.reverse();
        inventory.workspace_default_members.reverse();
        for package in &mut inventory.packages {
            package.dependencies.reverse();
        }
        let nodes = inventory.resolve.as_mut().expect("fixture resolve");
        nodes.reverse();
        for node in nodes {
            node.dependencies.reverse();
            for dependency in &mut node.dependencies {
                dependency.kinds.reverse();
            }
        }
    }

    #[test]
    fn diagnostic_writer_checks_remaining_bytes_before_appending_multibyte_text() {
        let allowance = MAX_DIAGNOSTIC_BYTES - TEXT_TRUNCATED.len();
        let mut text = DiagnosticText::new();
        text.write_str(&"x".repeat(allowance - 1))
            .expect("within allowance");
        let before = text.value.len();
        assert!(text.write_str("界").is_err());
        assert_eq!(text.value.len(), before);
        assert!(text.value.len() < MAX_DIAGNOSTIC_BYTES);

        let label = "界".repeat(MAX_LABEL_BYTES);
        let mut errors = Diagnostics::default();
        errors.push(format_args!("invalid package {:?}", Label(&label)));
        let errors = errors.finish();
        assert!(errors.iter().any(|error| error == DIAGNOSTICS_TRUNCATED));
        assert!(errors.iter().any(|error| error.contains(LABEL_TRUNCATED)));
        assert!(
            errors
                .iter()
                .all(|error| error.len() <= MAX_DIAGNOSTIC_BYTES)
        );
        assert!(Label(&label).prefix().len() + LABEL_TRUNCATED.len() <= MAX_LABEL_BYTES);
    }

    #[test]
    fn long_unicode_ids_and_many_long_paths_share_the_limits_and_stay_order_independent() {
        let mut inventory = inventory();
        // Each opaque ID stays below the adapter's 16 KiB text ceiling. The
        // chain has more workspace roots than the shared diagnostic budget.
        let suffix = "界".repeat(5459);
        let ids: Vec<_> = (0..MAX_DIAGNOSTICS + 8)
            .map(|index| format!("{index:04}-{suffix}"))
            .collect();
        for (position, id) in ids.iter().enumerate() {
            let next = ids.get(position + 1).map_or(SDK_ID, String::as_str);
            external(
                &mut inventory,
                id,
                "long-path-hop",
                vec![edge(next, DependencyKind::Normal)],
            );
            let package = package_mut(&mut inventory, id);
            package.source = None;
            package.manifest_path = PathBuf::from(format!("fixture/hop-{position}/Cargo.toml"));
            inventory.workspace_members.push(id.clone());
        }
        node_mut(&mut inventory, "app-id")
            .dependencies
            .push(edge(&ids[0], DependencyKind::Normal));
        let errors = validate_graph(&inventory, VERSION);
        assert_eq!(errors.len(), MAX_DIAGNOSTICS);
        assert!(
            errors
                .iter()
                .all(|error| error.len() <= MAX_DIAGNOSTIC_BYTES)
        );
        assert!(errors.iter().any(|error| error == DIAGNOSTICS_TRUNCATED));
        assert!(errors.iter().any(|error| error.contains(TEXT_TRUNCATED)));
        assert!(!errors.iter().any(|error| error.contains(&suffix)));
        reverse_inventory(&mut inventory);
        assert_eq!(validate_graph(&inventory, VERSION), errors);
    }

    #[test]
    fn invalid_edge_diagnostics_are_capped_before_growth_and_stable_after_reordering() {
        let mut inventory = inventory();
        let suffix = "界".repeat(5459);
        let app = node_mut(&mut inventory, "app-id");
        for index in 0..MAX_DIAGNOSTICS + 8 {
            let id = format!("{index:04}-{suffix}");
            app.dependencies.push(edge(&id, DependencyKind::Normal));
        }
        let errors = validate_graph(&inventory, VERSION);
        assert_eq!(errors.len(), MAX_DIAGNOSTICS);
        assert!(
            errors
                .iter()
                .all(|error| error.len() <= MAX_DIAGNOSTIC_BYTES)
        );
        assert!(errors.iter().any(|error| error == DIAGNOSTICS_TRUNCATED));
        reverse_inventory(&mut inventory);
        assert_eq!(validate_graph(&inventory, VERSION), errors);
    }

    #[test]
    fn qualified_bridge_is_a_leaf_and_private_registry_declarations_are_not_banned() {
        let mut inventory = inventory();
        let dependency = &mut package_mut(&mut inventory, "bridge-id").dependencies[0];
        dependency.rename = Some("private_sdk".to_owned());
        let bridge = node_mut(&mut inventory, "bridge-id");
        bridge.dependencies[0].name = "private_sdk".to_owned();
        external(
            &mut inventory,
            "helper-id",
            "transport-helper",
            vec![edge(SDK_ID, DependencyKind::Build)],
        );
        let mut private = declaration(DependencyKind::Build);
        private.requirement = "^1.19.0".to_owned();
        package_mut(&mut inventory, "helper-id")
            .dependencies
            .push(private);
        node_mut(&mut inventory, "bridge-id")
            .dependencies
            .push(edge("helper-id", DependencyKind::Normal));
        assert!(validate_graph(&inventory, VERSION).is_empty());
    }

    #[test]
    fn direct_workspace_declarations_are_forbidden_for_all_kinds_and_contexts() {
        for kind in [
            DependencyKind::Normal,
            DependencyKind::Build,
            DependencyKind::Development,
        ] {
            for restricted in [false, true] {
                let mut inventory = inventory();
                let mut dependency = declaration(kind);
                dependency.rename = Some("hidden_sdk".to_owned());
                dependency.optional = restricted;
                dependency.target = restricted.then(|| "cfg(target_os = \"none\")".to_owned());
                package_mut(&mut inventory, "app-id")
                    .dependencies
                    .push(dependency);
                node_mut(&mut inventory, "app-id")
                    .dependencies
                    .push(edge(SDK_ID, kind));
                assert_error(&inventory, "declares forbidden direct");
                let errors = validate_graph(&inventory, VERSION);
                assert_eq!(
                    errors
                        .iter()
                        .any(|error| error.contains("forbidden production dependency path")),
                    kind != DependencyKind::Development,
                );
            }
        }
    }

    #[test]
    fn shortest_path_includes_build_and_inactive_target_edges() {
        let mut inventory = inventory();
        external(
            &mut inventory,
            "long-1",
            "a-long",
            vec![edge("long-2", DependencyKind::Normal)],
        );
        external(
            &mut inventory,
            "long-2",
            "a-longer",
            vec![edge(SDK_ID, DependencyKind::Normal)],
        );
        external(
            &mut inventory,
            "short-id",
            "z-short",
            vec![edge(SDK_ID, DependencyKind::Build)],
        );
        let app = node_mut(&mut inventory, "app-id");
        app.dependencies
            .push(edge("long-1", DependencyKind::Normal));
        let mut short = edge("short-id", DependencyKind::Build);
        short.kinds[0].target = Some("cfg(target_os = \"none\")".to_owned());
        app.dependencies.push(short);
        let errors = validate_graph(&inventory, VERSION);
        assert_eq!(
            errors,
            vec![format!(
                "cargo metadata: forbidden production dependency path: application [app-id] -> \
             z-short [short-id] -> {VENDOR} [{SDK_ID}]",
            )]
        );
    }

    #[test]
    fn equally_short_paths_and_diagnostics_are_stable_under_input_permutation() {
        let mut inventory = inventory();
        external(
            &mut inventory,
            "z-alpha-id",
            "alpha",
            vec![edge(SDK_ID, DependencyKind::Normal)],
        );
        external(
            &mut inventory,
            "a-alpha-id",
            "alpha",
            vec![edge(SDK_ID, DependencyKind::Normal)],
        );
        external(
            &mut inventory,
            "a-beta-id",
            "beta",
            vec![edge(SDK_ID, DependencyKind::Normal)],
        );
        let app = node_mut(&mut inventory, "app-id");
        app.dependencies
            .push(edge("a-beta-id", DependencyKind::Normal));
        app.dependencies
            .push(edge("z-alpha-id", DependencyKind::Normal));
        app.dependencies
            .push(edge("a-alpha-id", DependencyKind::Normal));
        let errors = validate_graph(&inventory, VERSION);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("alpha [a-alpha-id]"));
        inventory.packages.reverse();
        inventory.workspace_members.reverse();
        inventory.workspace_default_members.reverse();
        let nodes = inventory.resolve.as_mut().expect("fixture resolve");
        nodes.reverse();
        for node in nodes {
            node.dependencies.reverse();
            for dependency in &mut node.dependencies {
                dependency.kinds.reverse();
            }
        }
        assert_eq!(validate_graph(&inventory, VERSION), errors);
    }

    #[test]
    fn development_only_transitive_path_is_ignored_but_a_mixed_edge_is_production() {
        let mut inventory = inventory();
        external(
            &mut inventory,
            "dev-helper",
            "test-helper",
            vec![edge(SDK_ID, DependencyKind::Normal)],
        );
        node_mut(&mut inventory, "app-id")
            .dependencies
            .push(edge("dev-helper", DependencyKind::Development));
        assert!(validate_graph(&inventory, VERSION).is_empty());
        node_mut(&mut inventory, "app-id")
            .dependencies
            .last_mut()
            .expect("dev edge")
            .kinds
            .push(CargoDependencyKind {
                kind: DependencyKind::Normal,
                target: Some("cfg(unix)".to_owned()),
            });
        assert_error(&inventory, "test-helper [dev-helper]");
    }

    #[test]
    fn another_package_named_bridge_does_not_gain_the_workspace_bridge_exemption() {
        let mut inventory = inventory();
        external(
            &mut inventory,
            "imposter-id",
            BRIDGE,
            vec![edge(SDK_ID, DependencyKind::Normal)],
        );
        // Even a registry package reporting the expected path is not the
        // bridge unless its exact package ID belongs to the workspace.
        package_mut(&mut inventory, "imposter-id").manifest_path =
            PathBuf::from("fixture").join(BRIDGE_MANIFEST);
        node_mut(&mut inventory, "app-id")
            .dependencies
            .push(edge("imposter-id", DependencyKind::Normal));
        assert_error(&inventory, "search-qdrant-bridge [imposter-id]");
    }

    #[test]
    fn workspace_bridge_name_does_not_authorize_another_manifest_path() {
        let mut relocated = inventory();
        package_mut(&mut relocated, "bridge-id").manifest_path =
            PathBuf::from("fixture/elsewhere/Cargo.toml");
        assert_error(&relocated, "expected one workspace search-qdrant-bridge at");
        assert_error(&relocated, "declares forbidden direct");
        assert_error(&relocated, "forbidden production dependency path");

        let mut renamed = inventory();
        package_mut(&mut renamed, "bridge-id").name = "unrelated-package".to_owned();
        assert_error(&renamed, "must name search-qdrant-bridge");
        assert_error(&renamed, "forbidden production dependency path");
    }

    #[test]
    fn missing_resolve_duplicate_ids_and_dangling_edges_fail_closed() {
        let valid = inventory();
        let mut missing = valid.clone();
        missing.resolve = None;
        assert_error(&missing, "full resolve graph is required");
        let mut duplicate = valid.clone();
        duplicate.packages.push(duplicate.packages[0].clone());
        assert_error(&duplicate, "duplicate package ID");
        let mut duplicate = valid.clone();
        let nodes = duplicate.resolve.as_mut().expect("fixture resolve");
        nodes.push(nodes[0].clone());
        assert_error(&duplicate, "duplicate resolve node ID");
        let errors = validate_graph(&duplicate, VERSION);
        let nodes = duplicate.resolve.as_mut().expect("fixture resolve");
        nodes
            .last_mut()
            .expect("duplicate node")
            .dependencies
            .push(edge("missing-id", DependencyKind::Normal));
        nodes.reverse();
        assert_eq!(validate_graph(&duplicate, VERSION), errors);
        let mut dangling = valid.clone();
        node_mut(&mut dangling, "app-id")
            .dependencies
            .push(edge("missing-id", DependencyKind::Development));
        assert_error(&dangling, "dangling resolve edge");
        let mut missing_node = valid.clone();
        missing_node
            .resolve
            .as_mut()
            .expect("fixture resolve")
            .retain(|node| node.id != SDK_ID);
        assert_error(&missing_node, "dangling resolve edge");
        let mut untyped = valid;
        node_mut(&mut untyped, "app-id").dependencies[0]
            .kinds
            .clear();
        assert_error(&untyped, "has no dependency kind");
    }

    #[test]
    fn workspace_and_default_membership_use_package_ids() {
        let valid = inventory();
        let mut duplicate = valid.clone();
        duplicate.workspace_members.push("app-id".to_owned());
        assert_error(&duplicate, "duplicate workspace member");
        let mut unknown = valid.clone();
        unknown.workspace_members.push("missing-id".to_owned());
        assert_error(&unknown, "lacks package/resolve node");
        let mut missing_node = valid.clone();
        missing_node
            .resolve
            .as_mut()
            .expect("fixture resolve")
            .retain(|node| node.id != "app-id");
        assert_error(
            &missing_node,
            "workspace member \"app-id\" lacks package/resolve node",
        );
        let mut nonmember = valid.clone();
        nonmember.workspace_default_members.push(SDK_ID.to_owned());
        assert_error(&nonmember, "is not a workspace member");
        let mut duplicate = valid;
        duplicate
            .workspace_default_members
            .push("app-id".to_owned());
        assert_error(&duplicate, "duplicate default member");
    }

    #[test]
    fn exact_vendor_version_and_registry_source_cannot_be_spoofed_by_the_id() {
        for source in [
            None,
            Some("git+https://example.invalid/qdrant#123"),
            Some("registry+https://example.invalid/index"),
        ] {
            let mut inventory = inventory();
            package_mut(&mut inventory, SDK_ID).source = source.map(str::to_owned);
            assert_error(&inventory, "must be version =1.19.0 from");
        }
        let mut wrong_version = inventory();
        package_mut(&mut wrong_version, SDK_ID).version = "1.19.1".to_owned();
        assert_error(&wrong_version, "found version 1.19.1");
        let mut duplicate = inventory();
        external(&mut duplicate, "second-sdk", VENDOR, Vec::new());
        assert_error(
            &duplicate,
            "expected one resolved qdrant-client package, found 2",
        );
    }

    #[test]
    fn bridge_declarations_reject_ranges_defaults_and_source_overrides() {
        for violation in [
            "range",
            "defaults",
            "path",
            "git",
            "registry",
            "explicit-registry",
            "default-feature",
            "feature",
            "optional",
            "target",
            "build",
            "development",
        ] {
            let mut inventory = inventory();
            let dependency = &mut package_mut(&mut inventory, "bridge-id").dependencies[0];
            match violation {
                "range" => dependency.requirement = "^1.19.0".to_owned(),
                "defaults" => dependency.uses_default_features = true,
                "path" => dependency.path = Some(PathBuf::from("fixture/local-sdk")),
                "git" => {
                    dependency.source = Some("git+https://example.invalid/qdrant#123".to_owned());
                }
                "registry" => {
                    dependency.registry = Some("https://example.invalid/index".to_owned());
                }
                "explicit-registry" => {
                    dependency.registry =
                        Some("https://github.com/rust-lang/crates.io-index".to_owned());
                }
                "default-feature" => dependency.features.push("default".to_owned()),
                "feature" => dependency.features.push("serde".to_owned()),
                "optional" => dependency.optional = true,
                "target" => dependency.target = Some("cfg(windows)".to_owned()),
                "build" => dependency.kind = DependencyKind::Build,
                "development" => dependency.kind = DependencyKind::Development,
                _ => unreachable!("closed fixture cases"),
            }
            assert_error(&inventory, "must use exact =1.19.0");
        }
        let mut inventory = inventory();
        node_mut(&mut inventory, SDK_ID)
            .features
            .push("default".to_owned());
        assert_error(&inventory, "must enable no features");
        node_mut(&mut inventory, SDK_ID).features = vec!["serde".to_owned()];
        assert_error(&inventory, "must enable no features");
    }

    #[test]
    fn bridge_requires_both_a_normal_declaration_and_a_normal_resolved_edge() {
        let mut declared = inventory();
        package_mut(&mut declared, "bridge-id").dependencies[0].kind = DependencyKind::Build;
        assert_error(&declared, "normal/unconditional/nonoptional");
        let mut resolved = inventory();
        node_mut(&mut resolved, "bridge-id").dependencies[0].kinds[0].kind =
            DependencyKind::Development;
        assert_error(
            &resolved,
            "lacks a resolved unconditional normal qdrant-client edge",
        );
        node_mut(&mut resolved, "bridge-id").dependencies[0].kinds[0] = CargoDependencyKind {
            kind: DependencyKind::Normal,
            target: Some("cfg(windows)".to_owned()),
        };
        assert_error(
            &resolved,
            "lacks a resolved unconditional normal qdrant-client edge",
        );
    }

    #[test]
    fn bridge_sdk_declaration_is_exactly_one_not_a_normal_plus_extra_alias() {
        let mut duplicate = inventory();
        let mut extra = declaration(DependencyKind::Normal);
        extra.rename = Some("second_sdk".to_owned());
        package_mut(&mut duplicate, "bridge-id")
            .dependencies
            .push(extra);
        assert_error(
            &duplicate,
            "must declare exactly one qdrant-client dependency, found 2",
        );

        let mut missing = inventory();
        package_mut(&mut missing, "bridge-id").dependencies.clear();
        assert_error(
            &missing,
            "must declare exactly one qdrant-client dependency, found 0",
        );
    }

    #[test]
    fn absent_or_registry_workspace_bridge_is_not_an_allowed_leaf() {
        let mut absent = inventory();
        absent.workspace_members.retain(|id| id != "bridge-id");
        assert_error(&absent, "expected one workspace search-qdrant-bridge at");
        assert_error(&absent, "forbidden production dependency path");
        let mut registry = inventory();
        package_mut(&mut registry, "bridge-id").source = Some(REGISTRY_SOURCE.to_owned());
        assert_error(
            &registry,
            "workspace search-qdrant-bridge must be a local package",
        );
        assert_error(&registry, "forbidden production dependency path");
    }
}
