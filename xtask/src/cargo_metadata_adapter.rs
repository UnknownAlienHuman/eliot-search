//! Official Cargo facts behind one bounded, read-only tooling adapter.
//!
//! Donor types stay here. Failed, oversized, non-UTF8 or unresolved output
//! never becomes an empty workspace or successful dependency graph.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cargo_metadata::{CargoOpt, MetadataCommand};

const MAX_PACKAGES: usize = 4096;
const MAX_EDGES: usize = 32_768;
const MAX_TEXT_BYTES: usize = 16 * 1024;

struct BoundedText(String);

impl std::fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self
            .0
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > MAX_TEXT_BYTES)
        {
            return Err(std::fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}

fn display(value: &impl std::fmt::Display) -> Result<String, String> {
    let mut sink = BoundedText(String::new());
    std::fmt::write(&mut sink, format_args!("{value}"))
        .map_err(|_| "metadata text byte limit exceeded")?;
    Ok(sink.0)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyKind {
    Normal,
    Build,
    Development,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoInventory {
    pub workspace_root: PathBuf,
    pub workspace_members: Vec<String>,
    pub workspace_default_members: Vec<String>,
    pub packages: Vec<CargoPackage>,
    pub resolve: Option<Vec<CargoNode>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub source: Option<String>,
    pub dependencies: Vec<CargoDependency>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoDependency {
    pub name: String,
    pub rename: Option<String>,
    pub requirement: String,
    pub kind: DependencyKind,
    pub optional: bool,
    pub target: Option<String>,
    pub source: Option<String>,
    pub registry: Option<String>,
    pub path: Option<PathBuf>,
    pub uses_default_features: bool,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoNode {
    pub id: String,
    pub dependencies: Vec<CargoResolvedDependency>,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoResolvedDependency {
    pub name: String,
    pub package_id: String,
    pub kinds: Vec<CargoDependencyKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoDependencyKind {
    pub kind: DependencyKind,
    pub target: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct RunLimits {
    stdout_bytes: usize,
    stderr_bytes: usize,
    timeout: Duration,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            stdout_bytes: 32 * 1024 * 1024,
            stderr_bytes: 1024 * 1024,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
struct CapturedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
enum Stream {
    Stdout,
    Stderr,
}

fn read_capped(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut captured = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("metadata pipe read failed: {error}")),
        };
        if read == 0 {
            return Ok(captured);
        }
        let length = captured
            .len()
            .checked_add(read)
            .ok_or("metadata output size overflow")?;
        if length > limit {
            return Err("metadata output byte limit exceeded".into());
        }
        // Check each read before any captured-output growth.
        captured.extend_from_slice(&chunk[..read]);
    }
}

fn stop_child(child: &mut Child) -> String {
    // Never block waiting for readers or descendants after the single deadline.
    // This read-only command fails terminally; it is not replayed.
    if matches!(child.try_wait(), Ok(Some(_))) {
        return String::new();
    }
    child.kill().map_or_else(
        |error| format!("; direct-child cleanup failed: {error}"),
        |()| String::new(),
    )
}

fn run_bounded(mut command: Command, limits: RunLimits) -> Result<CapturedOutput, String> {
    if limits.stdout_bytes == 0 || limits.stderr_bytes == 0 || limits.timeout.is_zero() {
        return Err("metadata execution limits must be nonzero".into());
    }
    let deadline = Instant::now()
        .checked_add(limits.timeout)
        .ok_or("metadata deadline overflow")?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("metadata command spawn failed: {error}"))?;
    let Some(stdout) = child.stdout.take() else {
        return Err(format!(
            "metadata stdout pipe missing{}",
            stop_child(&mut child)
        ));
    };
    let Some(stderr) = child.stderr.take() else {
        return Err(format!(
            "metadata stderr pipe missing{}",
            stop_child(&mut child)
        ));
    };
    let (sender, receiver) = mpsc::channel();
    let stdout_sender = sender.clone();
    // Exactly two readers send at most one bounded buffer each. A result queue
    // therefore cannot grow with the producer's output. No blocking joins.
    std::thread::Builder::new()
        .name("cargo-metadata-stdout".into())
        .spawn(move || {
            let _ = stdout_sender.send((Stream::Stdout, read_capped(stdout, limits.stdout_bytes)));
        })
        .map_err(|error| {
            format!(
                "metadata stdout reader spawn failed: {error}{}",
                stop_child(&mut child)
            )
        })?;
    std::thread::Builder::new()
        .name("cargo-metadata-stderr".into())
        .spawn(move || {
            let _ = sender.send((Stream::Stderr, read_capped(stderr, limits.stderr_bytes)));
        })
        .map_err(|error| {
            format!(
                "metadata stderr reader spawn failed: {error}{}",
                stop_child(&mut child)
            )
        })?;
    let mut stdout = None;
    let mut stderr = None;
    let mut status = None;
    loop {
        if Instant::now() >= deadline {
            return Err(format!(
                "metadata command deadline exceeded{}",
                stop_child(&mut child)
            ));
        }
        while let Ok((stream, result)) = receiver.try_recv() {
            let bytes = match result {
                Ok(bytes) => bytes,
                Err(error) => {
                    return Err(format!(
                        "metadata {stream:?}: {error}{}",
                        stop_child(&mut child)
                    ));
                }
            };
            match stream {
                Stream::Stdout => stdout = Some(bytes),
                Stream::Stderr => stderr = Some(bytes),
            }
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(status) => status,
                Err(error) => {
                    return Err(format!(
                        "metadata status read failed: {error}{}",
                        stop_child(&mut child)
                    ));
                }
            };
        }
        if let Some(exit) = status {
            if !exit.success() {
                return Err(format!("metadata command exited unsuccessfully: {exit}"));
            }
            if let (Some(stdout), Some(stderr)) = (&mut stdout, &mut stderr) {
                return Ok(CapturedOutput {
                    stdout: std::mem::take(stdout),
                    stderr: std::mem::take(stderr),
                });
            }
        }
        std::thread::sleep(
            Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

pub fn load_inventory(root: &Path, no_deps: bool) -> Result<CargoInventory, String> {
    // Resolve before setting both cwd and manifest path; a relative root must
    // not be applied twice by the subprocess.
    let expected = std::fs::canonicalize(root)
        .map_err(|error| format!("workspace root read failed: {error}"))?;
    let output = run_bounded(metadata_command(&expected, no_deps), RunLimits::default())?;
    let parsed = parse_output(&output)?;
    let inventory = into_inventory(parsed)?;
    let actual = std::fs::canonicalize(&inventory.workspace_root)
        .map_err(|error| format!("metadata workspace root read failed: {error}"))?;
    if expected != actual {
        return Err("metadata workspace root mismatch".into());
    }
    if !no_deps && inventory.resolve.is_none() {
        return Err("metadata full resolve graph missing".into());
    }
    Ok(inventory)
}

fn metadata_command(root: &Path, no_deps: bool) -> Command {
    let mut metadata = MetadataCommand::new();
    metadata
        .manifest_path(root.join("Cargo.toml"))
        .current_dir(root);
    metadata.other_options(vec!["--locked".into(), "--offline".into()]);
    if no_deps {
        metadata.no_deps();
    } else {
        metadata.features(CargoOpt::AllFeatures);
    }
    metadata.cargo_command()
}

fn parse_output(output: &CapturedOutput) -> Result<cargo_metadata::Metadata, String> {
    // Stderr has already been independently capped; it carries no inventory.
    let _stderr_bytes = output.stderr.len();
    let text = std::str::from_utf8(&output.stdout).map_err(|_| "metadata stdout is not UTF-8")?;
    MetadataCommand::parse(text).map_err(|error| format!("metadata parse failed: {error}"))
}

fn count(value: usize, limit: usize, field: &str) -> Result<(), String> {
    if value > limit {
        Err(format!("metadata {field} count exceeds {limit}"))
    } else {
        Ok(())
    }
}

fn text(value: &str) -> Result<(), String> {
    count(value.len(), MAX_TEXT_BYTES, "text bytes")
}

fn kind(value: cargo_metadata::DependencyKind) -> Result<DependencyKind, String> {
    match value {
        cargo_metadata::DependencyKind::Normal => Ok(DependencyKind::Normal),
        cargo_metadata::DependencyKind::Build => Ok(DependencyKind::Build),
        cargo_metadata::DependencyKind::Development => Ok(DependencyKind::Development),
        cargo_metadata::DependencyKind::Unknown => {
            Err("metadata dependency kind is unknown".into())
        }
    }
}

fn into_inventory(metadata: cargo_metadata::Metadata) -> Result<CargoInventory, String> {
    count(metadata.packages.len(), MAX_PACKAGES, "packages")?;
    count(
        metadata.workspace_members.len(),
        MAX_PACKAGES,
        "workspace members",
    )?;
    if metadata.workspace_default_members.is_missing() {
        return Err("metadata default workspace members missing".into());
    }
    count(
        metadata.workspace_default_members.len(),
        MAX_PACKAGES,
        "default members",
    )?;
    for id in metadata
        .workspace_members
        .iter()
        .chain(metadata.workspace_default_members.iter())
    {
        text(&id.repr)?;
    }
    let defaults = metadata
        .workspace_default_members
        .iter()
        .map(|id| id.repr.clone())
        .collect();
    let members = metadata
        .workspace_members
        .into_iter()
        .map(|id| id.repr)
        .collect();
    let packages = into_packages(metadata.packages)?;
    let resolve = metadata.resolve.map(into_nodes).transpose()?;
    let inventory = CargoInventory {
        workspace_root: metadata.workspace_root.into_std_path_buf(),
        workspace_members: members,
        workspace_default_members: defaults,
        packages,
        resolve,
    };
    validate_inventory_text(&inventory)?;
    Ok(inventory)
}

fn into_packages(
    donor_packages: Vec<cargo_metadata::Package>,
) -> Result<Vec<CargoPackage>, String> {
    let mut edges = 0_usize;
    let mut packages = Vec::new();
    for package in donor_packages {
        text(&package.id.repr)?;
        text(package.name.as_str())?;
        text(package.manifest_path.as_str())?;
        edges = edges
            .checked_add(package.dependencies.len())
            .ok_or("metadata edge count overflow")?;
        count(edges, MAX_EDGES, "declared edges")?;
        let mut dependencies = Vec::new();
        for dependency in package.dependencies {
            text(&dependency.name)?;
            if let Some(rename) = &dependency.rename {
                text(rename)?;
            }
            count(
                dependency.features.len(),
                MAX_PACKAGES,
                "dependency features",
            )?;
            for feature in &dependency.features {
                text(feature)?;
            }
            dependencies.push(CargoDependency {
                name: dependency.name,
                rename: dependency.rename,
                requirement: display(&dependency.req)?,
                kind: kind(dependency.kind)?,
                optional: dependency.optional,
                target: dependency.target.as_ref().map(display).transpose()?,
                source: dependency.source.as_ref().map(display).transpose()?,
                registry: dependency.registry,
                path: dependency
                    .path
                    .map(cargo_metadata::camino::Utf8PathBuf::into_std_path_buf),
                uses_default_features: dependency.uses_default_features,
                features: dependency.features,
            });
        }
        packages.push(CargoPackage {
            id: package.id.repr,
            name: display(&package.name)?,
            version: display(&package.version)?,
            manifest_path: package.manifest_path.into_std_path_buf(),
            source: package.source.as_ref().map(display).transpose()?,
            dependencies,
        });
    }
    Ok(packages)
}

fn into_nodes(resolve: cargo_metadata::Resolve) -> Result<Vec<CargoNode>, String> {
    count(resolve.nodes.len(), MAX_PACKAGES, "resolve nodes")?;
    let mut nodes = Vec::new();
    let mut edge_count = 0_usize;
    for node in resolve.nodes {
        text(&node.id.repr)?;
        count(node.features.len(), MAX_PACKAGES, "enabled features")?;
        edge_count = edge_count
            .checked_add(node.deps.len())
            .ok_or("metadata edge count overflow")?;
        count(edge_count, MAX_EDGES, "resolve edges")?;
        let mut dependencies = Vec::new();
        for dependency in node.deps {
            count(dependency.dep_kinds.len(), MAX_PACKAGES, "edge kinds")?;
            text(&dependency.name)?;
            text(&dependency.pkg.repr)?;
            let kinds = dependency
                .dep_kinds
                .into_iter()
                .map(|value| {
                    Ok(CargoDependencyKind {
                        kind: kind(value.kind)?,
                        target: value.target.as_ref().map(display).transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            dependencies.push(CargoResolvedDependency {
                name: dependency.name,
                package_id: dependency.pkg.repr,
                kinds,
            });
        }
        nodes.push(CargoNode {
            id: node.id.repr,
            dependencies,
            features: node
                .features
                .iter()
                .map(display)
                .collect::<Result<Vec<_>, String>>()?,
        });
    }
    Ok(nodes)
}

fn optional_text(value: Option<&str>) -> Result<(), String> {
    value.map_or(Ok(()), text)
}

fn path_text(value: &Path) -> Result<(), String> {
    text(value.to_str().ok_or("metadata path is not UTF-8")?)
}

fn validate_inventory_text(inventory: &CargoInventory) -> Result<(), String> {
    path_text(&inventory.workspace_root)?;
    for id in inventory
        .workspace_members
        .iter()
        .chain(&inventory.workspace_default_members)
    {
        text(id)?;
    }
    for package in &inventory.packages {
        text(&package.version)?;
        optional_text(package.source.as_deref())?;
        for dependency in &package.dependencies {
            text(&dependency.requirement)?;
            optional_text(dependency.target.as_deref())?;
            optional_text(dependency.source.as_deref())?;
            optional_text(dependency.registry.as_deref())?;
            if let Some(path) = &dependency.path {
                path_text(path)?;
            }
        }
    }
    for node in inventory.resolve.iter().flatten() {
        for feature in &node.features {
            text(feature)?;
        }
        for dependency in &node.dependencies {
            for kind in &dependency.kinds {
                optional_text(kind.target.as_deref())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod process_fixture;

#[cfg(test)]
mod inventory_fixture;
