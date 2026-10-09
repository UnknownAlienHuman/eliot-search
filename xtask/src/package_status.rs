//! Bounded schema-v3 documentation inventory validation, not qualification.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use manifest_toml::Value;

use crate::cargo_metadata_adapter::CargoInventory;

const STATUS_FILE: &str = "docs/product/PACKAGE_STATUS.toml";
const MAX_STATUS_BYTES: usize = 1_048_576;
const MAX_READ_BYTES: u64 = 1_048_577;
const MAX_ITEMS: usize = 4_096;
const MAX_FIELD_BYTES: usize = 4_096;
const MAX_ERRORS: usize = 128;
const MAX_ERROR_BYTES: usize = 512;
const ROOT_FIELDS: &[&str] = &[
    "schema_version",
    "snapshot_date",
    "snapshot_source_sha",
    "coordinator",
    "documentation_owner",
    "defaults",
    "package",
];
const STATUS_FIELDS: &[&str] = &[
    "contract_defined",
    "source_present",
    "product_path",
    "check_clippy_evidence",
    "live_native_qualified",
    "public_capability_enabled",
    "disposition",
];
const ROW_FIELDS: &[&str] = &[
    "name",
    "path",
    "execution_chain",
    "consumers",
    "optional_followups",
    "contract_defined",
    "source_present",
    "product_path",
    "check_clippy_evidence",
    "live_native_qualified",
    "public_capability_enabled",
    "disposition",
];
const PRODUCT_PATHS: &[&str] = &[
    "not_asserted",
    "absent",
    "partial",
    "integrated",
    "legacy_only",
];
const EVIDENCE: &[&str] = &[
    "not_asserted",
    "not_run",
    "historical_pass",
    "historical_fail",
    "exact_pass",
    "exact_fail",
];
const DISPOSITIONS: &[&str] = &[
    "active",
    "legacy_profile",
    "optional_disabled",
    "evaluation_only",
    "tooling_only",
];

#[derive(Default)]
struct Diagnostics {
    errors: Vec<String>,
    truncated: bool,
}

impl Diagnostics {
    fn push(&mut self, message: impl AsRef<str>) {
        if self.errors.len() >= MAX_ERRORS {
            self.truncated = true;
            return;
        }
        let message = message.as_ref();
        let mut end = message.len().min(MAX_ERROR_BYTES);
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        self.errors.push(message[..end].to_owned());
    }

    fn finish(mut self) -> Vec<String> {
        if self.truncated {
            self.errors.pop();
            self.errors
                .push("PACKAGE_STATUS: additional validation errors omitted at limit".into());
        }
        self.errors.sort_unstable();
        self.errors.dedup();
        self.errors
    }
}

fn label(value: &str) -> String {
    value.chars().take(96).collect()
}

/// Validates status and Cargo workspace member READMEs, with a one-MiB cap per file.
#[must_use]
pub fn validate_package_status(root: &Path, inventory: &CargoInventory) -> Vec<String> {
    let bytes = match read_status(root) {
        Ok(bytes) => bytes,
        Err(error) => return vec![error],
    };
    let status_errors = String::from_utf8(bytes).map_or_else(
        |_| vec!["PACKAGE_STATUS: status file must be UTF-8".into()],
        |text| validate_status_text(&text, inventory),
    );
    let mut errors = Diagnostics::default();
    for message in status_errors {
        errors.push(message);
    }
    scan_readmes(inventory, &mut errors);
    errors.finish()
}

fn read_status(root: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(root.join(STATUS_FILE)).map_err(|error| {
        format!(
            "PACKAGE_STATUS: cannot open status file: {}",
            label(&error.to_string())
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        format!(
            "PACKAGE_STATUS: cannot inspect status file: {}",
            label(&error.to_string())
        )
    })?;
    if !metadata.is_file() {
        return Err("PACKAGE_STATUS: status input must be a regular file".into());
    }
    if metadata.len() >= MAX_READ_BYTES {
        return Err("PACKAGE_STATUS: status file exceeds one MiB".into());
    }
    read_capped(file, "status file")
}

fn read_capped(reader: impl Read, kind: &'static str) -> Result<Vec<u8>, String> {
    let mut reader = reader.take(MAX_READ_BYTES);
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8_192];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Ok(bytes),
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(format!(
                    "PACKAGE_STATUS: cannot read {kind}: {}",
                    label(&error.to_string())
                ));
            }
        };
        if read > MAX_STATUS_BYTES - bytes.len() {
            return Err(format!("PACKAGE_STATUS: {kind} exceeds one MiB"));
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

fn read_readme(directory: &Path) -> Result<Option<String>, String> {
    let file = match File::open(directory.join("README.md")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot open README: {}", label(&error.to_string()))),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect README: {}", label(&error.to_string())))?;
    if !metadata.is_file() {
        return Err("README input must be a regular file".into());
    }
    if metadata.len() >= MAX_READ_BYTES {
        return Err("README exceeds one MiB".into());
    }
    let bytes = read_capped(file, "README")?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "README must be UTF-8".into())
}

fn has_banned_readme_status(text: &str) -> bool {
    // Borrowed word tokens normalize whitespace (including line breaks) and
    // Markdown punctuation without allocating a second normalized document.
    let mut previous_intentionally = false;
    for token in text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        if previous_intentionally && token.eq_ignore_ascii_case("unimplemented") {
            return true;
        }
        previous_intentionally = token.eq_ignore_ascii_case("intentionally");
    }
    false
}

fn scan_readmes(inventory: &CargoInventory, errors: &mut Diagnostics) {
    let Some(members) = workspace_paths(inventory, errors) else {
        return;
    };
    // All active Cargo members participate, including optional, evaluation
    // and tooling packages; documentation disposition grants no exemption.
    for (name, path) in members {
        match read_readme(&inventory.workspace_root.join(&path)) {
            Ok(Some(text)) if has_banned_readme_status(&text) => {
                errors.push(format!(
                    "PACKAGE_STATUS: {}/README.md ({}) contains banned phrase 'intentionally unimplemented'",
                    label(&path), label(name),
                ));
            }
            Ok(_) => {}
            Err(error) => {
                errors.push(format!(
                    "PACKAGE_STATUS: {}/README.md: {}",
                    label(&path),
                    label(&error)
                ));
            }
        }
    }
}

/// Uses Cargo-owned workspace membership; no manifest or dependency inference.
#[must_use]
pub fn validate_status_text(text: &str, inventory: &CargoInventory) -> Vec<String> {
    let mut errors = Diagnostics::default();
    if text.len() > MAX_STATUS_BYTES {
        errors.push("PACKAGE_STATUS: status text exceeds one MiB");
        return errors.finish();
    }
    let Ok(document) = manifest_toml::from_str::<Value>(text) else {
        errors.push("PACKAGE_STATUS: invalid TOML");
        return errors.finish();
    };
    if !closed(&document, ROOT_FIELDS, "root", &mut errors) {
        return errors.finish();
    }
    validate_header(&document, &mut errors);
    let Some(defaults) = document.get("defaults") else {
        errors.push("PACKAGE_STATUS: defaults table is required");
        return errors.finish();
    };
    if !closed(defaults, STATUS_FIELDS, "defaults", &mut errors)
        || status(defaults, defaults, "defaults", &mut errors).is_none()
    {
        return errors.finish();
    }
    let Some(expected) = workspace_paths(inventory, &mut errors) else {
        return errors.finish();
    };
    let Some(rows) = document.get("package").and_then(Value::as_array) else {
        errors.push("PACKAGE_STATUS: package must be an array of tables");
        return errors.finish();
    };
    if rows.len() > MAX_ITEMS {
        errors.push("PACKAGE_STATUS: package row limit exceeded");
        return errors.finish();
    }
    validate_rows(rows, defaults, &expected, &mut errors);
    errors.finish()
}

fn closed(value: &Value, allowed: &[&str], location: &str, errors: &mut Diagnostics) -> bool {
    let Some(table) = value.as_table() else {
        errors.push(format!("PACKAGE_STATUS: {location} must be a table"));
        return false;
    };
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            errors.push(format!(
                "PACKAGE_STATUS: {location} has unknown field {}",
                label(key)
            ));
        }
    }
    true
}

fn string<'a>(
    value: Option<&'a Value>,
    location: &str,
    errors: &mut Diagnostics,
) -> Option<&'a str> {
    match value.and_then(Value::as_str) {
        Some(value) if !value.is_empty() && value.len() <= MAX_FIELD_BYTES => Some(value),
        _ => {
            errors.push(format!(
                "PACKAGE_STATUS: {location} must be nonempty bounded text"
            ));
            None
        }
    }
}

fn reference(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(|digits| {
        !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && digits.parse::<u64>().is_ok_and(|number| number > 0)
    })
}

fn date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        value[..4].parse::<u16>(),
        value[5..7].parse::<u8>(),
        value[8..].parse::<u8>(),
    ) else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 0,
    };
    year > 0 && day > 0 && day <= days
}

fn validate_header(document: &Value, errors: &mut Diagnostics) {
    if document.get("schema_version").and_then(Value::as_integer) != Some(3) {
        errors.push("PACKAGE_STATUS: schema_version must be integer 3");
    }
    if let Some(value) = string(document.get("snapshot_date"), "snapshot_date", errors)
        && !date(value)
    {
        errors.push("PACKAGE_STATUS: snapshot_date must be a valid YYYY-MM-DD date");
    }
    if let Some(value) = string(
        document.get("snapshot_source_sha"),
        "snapshot_source_sha",
        errors,
    ) && (value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        errors.push("PACKAGE_STATUS: snapshot_source_sha must contain 40 hexadecimal digits");
    }
    for key in ["coordinator", "documentation_owner"] {
        if let Some(value) = string(document.get(key), key, errors)
            && !reference(value)
        {
            errors.push(format!(
                "PACKAGE_STATUS: {key} must be a #positive-integer reference"
            ));
        }
    }
}

fn references(value: Option<&Value>, required: bool, location: &str, errors: &mut Diagnostics) {
    if value.is_none() && !required {
        return;
    }
    let Some(values) = value.and_then(Value::as_array) else {
        errors.push(format!(
            "PACKAGE_STATUS: {location} must be an array of references"
        ));
        return;
    };
    if values.len() > MAX_ITEMS || (required && values.is_empty()) {
        errors.push(format!(
            "PACKAGE_STATUS: {location} must contain {}bounded references",
            if required { "nonempty " } else { "" }
        ));
        return;
    }
    for (index, value) in values.iter().enumerate() {
        if !value.as_str().is_some_and(reference) {
            errors.push(format!(
                "PACKAGE_STATUS: {location}[{index}] must be a #positive-integer reference"
            ));
        }
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent schema-v3 documentation flags."
)]
struct Status<'a> {
    contract_defined: bool,
    source_present: bool,
    product_path: &'a str,
    live_native_qualified: bool,
    public_capability_enabled: bool,
    disposition: &'a str,
}

fn flag(value: Option<&Value>, location: &str, errors: &mut Diagnostics) -> Option<bool> {
    value.and_then(Value::as_bool).map_or_else(
        || {
            errors.push(format!("PACKAGE_STATUS: {location} must be boolean"));
            None
        },
        Some,
    )
}

fn domain<'a>(
    value: Option<&'a Value>,
    allowed: &[&str],
    location: &str,
    errors: &mut Diagnostics,
) -> Option<&'a str> {
    let value = string(value, location, errors)?;
    if !allowed.contains(&value) {
        errors.push(format!(
            "PACKAGE_STATUS: {location} has unknown status {}",
            label(value)
        ));
        return None;
    }
    Some(value)
}

fn status<'a>(
    row: &'a Value,
    defaults: &'a Value,
    location: &str,
    errors: &mut Diagnostics,
) -> Option<Status<'a>> {
    let get = |key| row.get(key).or_else(|| defaults.get(key));
    let contract_defined = flag(
        get("contract_defined"),
        &format!("{location}.contract_defined"),
        errors,
    )?;
    domain(
        get("check_clippy_evidence"),
        EVIDENCE,
        &format!("{location}.check_clippy_evidence"),
        errors,
    )?;
    Some(Status {
        contract_defined,
        source_present: flag(
            get("source_present"),
            &format!("{location}.source_present"),
            errors,
        )?,
        product_path: domain(
            get("product_path"),
            PRODUCT_PATHS,
            &format!("{location}.product_path"),
            errors,
        )?,
        live_native_qualified: flag(
            get("live_native_qualified"),
            &format!("{location}.live_native_qualified"),
            errors,
        )?,
        public_capability_enabled: flag(
            get("public_capability_enabled"),
            &format!("{location}.public_capability_enabled"),
            errors,
        )?,
        disposition: domain(
            get("disposition"),
            DISPOSITIONS,
            &format!("{location}.disposition"),
            errors,
        )?,
    })
}

fn consistency(status: &Status<'_>, location: &str, errors: &mut Diagnostics) {
    if status.disposition != "active" && status.public_capability_enabled {
        errors.push(format!(
            "PACKAGE_STATUS: {location} non-active disposition cannot enable public capability"
        ));
    }
    if status.disposition != "active" && status.product_path == "integrated" {
        errors.push(format!(
            "PACKAGE_STATUS: {location} non-active disposition cannot be integrated"
        ));
    }
    if !status.contract_defined
        && (status.product_path == "integrated" || status.public_capability_enabled)
    {
        errors.push(format!(
            "PACKAGE_STATUS: {location} undefined contract cannot be integrated or publicly enabled"
        ));
    }
    if status.public_capability_enabled
        && (status.product_path != "integrated" || !status.live_native_qualified)
    {
        errors.push(format!("PACKAGE_STATUS: {location} public enablement requires integrated and live_native_qualified"));
    }
    if !status.source_present
        && (status.product_path == "integrated" || status.public_capability_enabled)
    {
        errors.push(format!(
            "PACKAGE_STATUS: {location} absent source cannot be integrated or publicly enabled"
        ));
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FIELD_BYTES
        && !value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '/' | '\\' | ':')
        })
}

fn valid_path(value: &str) -> bool {
    value == "."
        || (!value.is_empty()
            && value.len() <= MAX_FIELD_BYTES
            && !value.contains(['\\', ':'])
            && !value.chars().any(char::is_control)
            && value
                .split('/')
                .all(|part| !matches!(part, "" | "." | "..")))
}

fn package_path(root: &Path, manifest: &Path) -> Option<String> {
    if !manifest.is_absolute() || manifest.file_name()? != "Cargo.toml" {
        return None;
    }
    let parent = manifest.strip_prefix(root).ok()?.parent()?;
    let mut components = Vec::new();
    for component in parent.components() {
        let Component::Normal(value) = component else {
            return None;
        };
        components.push(value.to_str()?);
    }
    let relative = if components.is_empty() {
        ".".into()
    } else {
        components.join("/")
    };
    valid_path(&relative).then_some(relative)
}

fn workspace_paths<'a>(
    inventory: &'a CargoInventory,
    errors: &mut Diagnostics,
) -> Option<BTreeMap<&'a str, String>> {
    if !inventory.workspace_root.is_absolute()
        || inventory.packages.len() > MAX_ITEMS
        || inventory.workspace_members.len() > MAX_ITEMS
        || inventory.workspace_default_members.len() > MAX_ITEMS
    {
        errors.push("PACKAGE_STATUS: Cargo inventory must have an absolute root and bounded package/member lists");
        return None;
    }
    let mut valid = true;
    let mut packages = BTreeMap::new();
    for package in &inventory.packages {
        if packages.insert(package.id.as_str(), package).is_some() {
            errors.push("PACKAGE_STATUS: Cargo inventory has duplicate package IDs");
            valid = false;
        }
    }
    let mut members = BTreeSet::new();
    let mut expected = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for id in &inventory.workspace_members {
        if !members.insert(id.as_str()) {
            errors.push("PACKAGE_STATUS: Cargo inventory has duplicate workspace members");
            valid = false;
        }
        let Some(package) = packages.get(id.as_str()) else {
            errors.push("PACKAGE_STATUS: Cargo workspace member has no package record");
            valid = false;
            continue;
        };
        if !valid_name(&package.name) {
            errors.push("PACKAGE_STATUS: Cargo workspace member has an invalid bounded name");
            valid = false;
            continue;
        }
        let Some(path) = package_path(&inventory.workspace_root, &package.manifest_path) else {
            errors.push(format!(
                "PACKAGE_STATUS: Cargo member {} manifest must be root-relative Cargo.toml",
                label(&package.name)
            ));
            valid = false;
            continue;
        };
        if !paths.insert(path.clone()) || expected.insert(package.name.as_str(), path).is_some() {
            errors.push("PACKAGE_STATUS: Cargo workspace has duplicate member names or paths");
            valid = false;
        }
    }
    let mut defaults = BTreeSet::new();
    for id in &inventory.workspace_default_members {
        if !defaults.insert(id.as_str()) || !members.contains(id.as_str()) {
            errors.push("PACKAGE_STATUS: Cargo default members must be unique workspace members");
            valid = false;
        }
    }
    valid.then_some(expected)
}

fn validate_rows(
    rows: &[Value],
    defaults: &Value,
    expected: &BTreeMap<&str, String>,
    errors: &mut Diagnostics,
) {
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let location = format!("package[{index}]");
        if !closed(row, ROW_FIELDS, &location, errors) {
            continue;
        }
        let name = string(row.get("name"), &format!("{location}.name"), errors);
        let path = string(row.get("path"), &format!("{location}.path"), errors);
        if let Some(name) = name {
            if !valid_name(name) {
                errors.push(format!("PACKAGE_STATUS: {location}.name is invalid"));
            }
            if !names.insert(name) {
                errors.push(format!(
                    "PACKAGE_STATUS: duplicate package row {}",
                    label(name)
                ));
            }
            match expected.get(name) {
                None => errors.push(format!("PACKAGE_STATUS: extra package row {}", label(name))),
                Some(expected_path) if path.is_some_and(|path| path != expected_path.as_str()) => {
                    errors.push(format!(
                        "PACKAGE_STATUS: {location}.path does not match Cargo workspace member"
                    ));
                }
                Some(_) => {}
            }
        }
        if let Some(path) = path {
            if !valid_path(path) {
                errors.push(format!(
                    "PACKAGE_STATUS: {location}.path must be a canonical root-relative '/' path"
                ));
            }
            if !paths.insert(path) {
                errors.push(format!(
                    "PACKAGE_STATUS: duplicate package path in {location}"
                ));
            }
        }
        references(
            row.get("execution_chain"),
            true,
            &format!("{location}.execution_chain"),
            errors,
        );
        for key in ["consumers", "optional_followups"] {
            references(row.get(key), false, &format!("{location}.{key}"), errors);
        }
        if let Some(status) = status(row, defaults, &location, errors) {
            consistency(&status, &location, errors);
        }
    }
    for name in expected.keys() {
        if !names.contains(name) {
            errors.push(format!(
                "PACKAGE_STATUS: missing package row {}",
                label(name)
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo_metadata_adapter::CargoPackage;

    const VALID: &str = r##"schema_version = 3
snapshot_date = "2026-10-09"
snapshot_source_sha = "889f144b55ff09a9412ed1f461e97b5e084211aa"
coordinator = "#97"
documentation_owner = "#97"

[defaults]
contract_defined = true
source_present = true
product_path = "not_asserted"
check_clippy_evidence = "not_asserted"
live_native_qualified = false
public_capability_enabled = false
disposition = "active"

[[package]]
name = "alpha"
path = "crates/alpha"
execution_chain = ["#250"]
consumers = ["#251"]
optional_followups = ["#214"]
"##;

    const EXTRA_ROW: &str = r##"
[[package]]
name = "beta"
path = "crates/beta"
execution_chain = ["#251"]
"##;

    fn inventory() -> CargoInventory {
        let root = std::env::temp_dir().join("eliot-package-status-fixture");
        CargoInventory {
            workspace_root: root.clone(),
            workspace_members: vec!["alpha-id".into()],
            workspace_default_members: vec!["alpha-id".into()],
            packages: vec![CargoPackage {
                id: "alpha-id".into(),
                name: "alpha".into(),
                version: "0.0.0".into(),
                manifest_path: root.join("crates/alpha/Cargo.toml"),
                source: None,
                dependencies: Vec::new(),
            }],
            resolve: None,
        }
    }

    fn accepted(text: &str, inventory: &CargoInventory) {
        let errors = validate_status_text(text, inventory);
        assert!(errors.is_empty(), "valid fixture rejected: {errors:?}");
    }

    fn rejected(text: &str, inventory: &CargoInventory, fragment: &str) {
        let errors = validate_status_text(text, inventory);
        assert!(
            errors.iter().any(|error| error.contains(fragment)),
            "missing {fragment:?}: {errors:?}"
        );
    }

    fn mutation(base: &str, from: &str, to: &str, fragment: &str) {
        let inventory = inventory();
        accepted(base, &inventory);
        assert!(base.contains(from), "mutation target must exist");
        let changed = base.replacen(from, to, 1);
        assert_ne!(changed, base);
        rejected(&changed, &inventory, fragment);
    }

    #[test]
    fn valid_defaults_and_typed_overrides_are_applied() {
        let inventory = inventory();
        accepted(VALID, &inventory);
        let overrides = format!(
            "{VALID}source_present = false\nproduct_path = \"absent\"\ndisposition = \"optional_disabled\"\n"
        );
        accepted(&overrides, &inventory);
        let enabled = qualified_enabled();
        accepted(&enabled, &inventory);
        for disposition in DISPOSITIONS {
            let text = VALID.replacen(
                "disposition = \"active\"",
                &format!("disposition = {disposition:?}"),
                1,
            );
            accepted(&text, &inventory);
        }
        for evidence in EVIDENCE {
            let text = VALID.replacen(
                "check_clippy_evidence = \"not_asserted\"",
                &format!("check_clippy_evidence = {evidence:?}"),
                1,
            );
            accepted(&text, &inventory);
        }
    }

    fn qualified_enabled() -> String {
        VALID
            .replacen(
                "product_path = \"not_asserted\"",
                "product_path = \"integrated\"",
                1,
            )
            .replacen(
                "live_native_qualified = false",
                "live_native_qualified = true",
                1,
            )
            .replacen(
                "public_capability_enabled = false",
                "public_capability_enabled = true",
                1,
            )
    }

    #[test]
    fn membership_missing_extra_duplicate_and_wrong_name_or_path_are_rejected() {
        let inventory = inventory();
        accepted(VALID, &inventory);
        let header = VALID.split("\n[[package]]").next().expect("header");
        let missing = header.replacen("[defaults]", "package = []\n[defaults]", 1);
        rejected(&missing, &inventory, "missing package row alpha");
        rejected(
            &format!("{VALID}{EXTRA_ROW}"),
            &inventory,
            "extra package row beta",
        );
        let row = VALID.split("\n[[package]]").nth(1).expect("row");
        rejected(
            &format!("{VALID}\n[[package]]{row}"),
            &inventory,
            "duplicate package row alpha",
        );
        mutation(
            VALID,
            "name = \"alpha\"",
            "name = \"beta\"",
            "extra package row beta",
        );
        mutation(
            VALID,
            "path = \"crates/alpha\"",
            "path = \"crates/beta\"",
            "path does not match Cargo",
        );
    }

    #[test]
    fn paths_reject_absolute_parent_alias_and_noncanonical_separators() {
        for path in [
            "/absolute",
            "C:/absolute",
            "../alpha",
            "crates/../alpha",
            "crates\\alpha",
            "crates//alpha",
            "crates/./alpha",
            "crates/alpha/",
        ] {
            mutation(
                VALID,
                "path = \"crates/alpha\"",
                &format!("path = {path:?}"),
                "canonical root-relative",
            );
        }
        mutation(
            VALID,
            "name = \"alpha\"",
            "name = \"alpha/beta\"",
            ".name is invalid",
        );
        mutation(
            VALID,
            "name = \"alpha\"",
            "name = \"\"",
            ".name must be nonempty",
        );
        mutation(
            VALID,
            "path = \"crates/alpha\"",
            "path = 1",
            ".path must be nonempty",
        );
    }

    #[test]
    fn root_package_uses_dot_and_manifest_escape_is_terminal() {
        let mut inventory = inventory();
        inventory.packages[0].manifest_path = inventory.workspace_root.join("Cargo.toml");
        let root_package = VALID.replacen("path = \"crates/alpha\"", "path = \".\"", 1);
        accepted(&root_package, &inventory);
        inventory.packages[0].manifest_path =
            inventory.workspace_root.join("../outside/Cargo.toml");
        rejected(
            &root_package,
            &inventory,
            "manifest must be root-relative Cargo.toml",
        );
    }

    #[test]
    fn unknown_fields_and_missing_required_fields_fail_closed() {
        mutation(
            VALID,
            "schema_version = 3",
            "schema_version = 3\nextra = true",
            "root has unknown field extra",
        );
        mutation(
            VALID,
            "[defaults]",
            "[defaults]\nextra = true",
            "defaults has unknown field extra",
        );
        mutation(
            VALID,
            "name = \"alpha\"",
            "name = \"alpha\"\nextra = true",
            "package[0] has unknown field extra",
        );
        for (field, fragment) in [
            ("name = \"alpha\"\n", ".name must be nonempty"),
            ("path = \"crates/alpha\"\n", ".path must be nonempty"),
            (
                "execution_chain = [\"#250\"]\n",
                ".execution_chain must be an array",
            ),
            (
                "source_present = true\n",
                "defaults.source_present must be boolean",
            ),
            (
                "snapshot_date = \"2026-10-09\"\n",
                "snapshot_date must be nonempty",
            ),
        ] {
            mutation(VALID, field, "", fragment);
        }
        mutation(
            VALID,
            "schema_version = 3",
            "schema_version = \"3\"",
            "schema_version must be integer 3",
        );
        mutation(
            VALID,
            "[[package]]",
            "[package]",
            "package must be an array",
        );
    }

    #[test]
    fn flags_are_boolean_in_defaults_and_in_overrides() {
        for (field, value) in [
            ("contract_defined", "true"),
            ("source_present", "true"),
            ("live_native_qualified", "false"),
            ("public_capability_enabled", "false"),
        ] {
            let from = format!("{field} = {value}");
            let to = format!("{field} = \"{value}\"");
            mutation(
                VALID,
                &from,
                &to,
                &format!("defaults.{field} must be boolean"),
            );
            let override_text = format!("{VALID}{to}\n");
            accepted(VALID, &inventory());
            rejected(
                &override_text,
                &inventory(),
                &format!("package[0].{field} must be boolean"),
            );
        }
    }

    #[test]
    fn all_status_domains_reject_unknown_values() {
        for field in ["product_path", "check_clippy_evidence", "disposition"] {
            let original = if field == "disposition" {
                "active"
            } else {
                "not_asserted"
            };
            mutation(
                VALID,
                &format!("{field} = {original:?}"),
                &format!("{field} = \"unknown\""),
                "unknown status unknown",
            );
            let text = format!("{VALID}{field} = \"unknown\"\n");
            rejected(&text, &inventory(), "unknown status unknown");
        }
    }

    #[test]
    fn every_reference_surface_rejects_bad_text_wrong_types_and_empty_execution() {
        for reference in ["#0", "#-1", "250", "#x", "#18446744073709551616"] {
            mutation(
                VALID,
                "execution_chain = [\"#250\"]",
                &format!("execution_chain = [{reference:?}]"),
                "execution_chain[0] must be a #positive",
            );
        }
        for field in ["consumers", "optional_followups"] {
            let number = if field == "consumers" { "#251" } else { "#214" };
            let from = format!("{field} = [{number:?}]");
            mutation(
                VALID,
                &from,
                &format!("{field} = [1]"),
                &format!("{field}[0] must be a #positive"),
            );
            mutation(
                VALID,
                &from,
                &format!("{field} = false"),
                &format!("{field} must be an array"),
            );
        }
        for field in ["coordinator", "documentation_owner"] {
            mutation(
                VALID,
                &format!("{field} = \"#97\""),
                &format!("{field} = \"#0\""),
                &format!("{field} must be a #positive"),
            );
        }
        mutation(
            VALID,
            "execution_chain = [\"#250\"]",
            "execution_chain = []",
            "execution_chain must contain nonempty",
        );
    }

    #[test]
    fn snapshot_date_and_sha_have_closed_domains() {
        mutation(
            VALID,
            "snapshot_date = \"2026-10-09\"",
            "snapshot_date = \"2026-02-30\"",
            "valid YYYY-MM-DD",
        );
        mutation(
            VALID,
            "snapshot_source_sha = \"889f144b55ff09a9412ed1f461e97b5e084211aa\"",
            "snapshot_source_sha = \"unknown\"",
            "40 hexadecimal digits",
        );
        assert!(date("2024-02-29"));
        assert!(!date("2025-02-29"));
    }

    #[test]
    fn disposition_enablement_and_absent_source_consistency_are_enforced() {
        for disposition in [
            "legacy_profile",
            "optional_disabled",
            "evaluation_only",
            "tooling_only",
        ] {
            let disabled = VALID.replacen(
                "disposition = \"active\"",
                &format!("disposition = {disposition:?}"),
                1,
            );
            mutation(
                &disabled,
                "product_path = \"not_asserted\"",
                "product_path = \"integrated\"",
                "cannot be integrated",
            );
        }
        for disposition in [
            "legacy_profile",
            "optional_disabled",
            "evaluation_only",
            "tooling_only",
        ] {
            let disabled = VALID.replacen(
                "disposition = \"active\"",
                &format!("disposition = {disposition:?}"),
                1,
            );
            mutation(
                &disabled,
                "public_capability_enabled = false",
                "public_capability_enabled = true",
                "non-active disposition cannot enable",
            );
        }
        let enabled = qualified_enabled();
        mutation(
            &enabled,
            "product_path = \"integrated\"",
            "product_path = \"partial\"",
            "enablement requires integrated",
        );
        mutation(
            &enabled,
            "live_native_qualified = true",
            "live_native_qualified = false",
            "enablement requires integrated",
        );
        mutation(
            &enabled,
            "source_present = true",
            "source_present = false",
            "absent source cannot be integrated",
        );
        let integrated = VALID.replacen(
            "product_path = \"not_asserted\"",
            "product_path = \"integrated\"",
            1,
        );
        mutation(
            &integrated,
            "source_present = true",
            "source_present = false",
            "absent source cannot be integrated",
        );
    }

    #[test]
    fn undefined_contract_cannot_be_integrated_or_enabled() {
        let undefined = VALID.replacen("contract_defined = true", "contract_defined = false", 1);
        accepted(&undefined, &inventory());
        mutation(
            &undefined,
            "product_path = \"not_asserted\"",
            "product_path = \"integrated\"",
            "undefined contract cannot be integrated",
        );
        let enabled = qualified_enabled();
        mutation(
            &enabled,
            "contract_defined = true",
            "contract_defined = false",
            "undefined contract cannot be integrated",
        );
    }

    #[test]
    fn malformed_inventory_never_becomes_an_empty_successful_workspace() {
        let good = inventory();
        accepted(VALID, &good);
        let mut bad = good.clone();
        bad.workspace_default_members.push("unknown-id".into());
        rejected(
            VALID,
            &bad,
            "default members must be unique workspace members",
        );
        let mut bad = good.clone();
        bad.workspace_members.push("alpha-id".into());
        rejected(VALID, &bad, "duplicate workspace members");
        let mut bad = good.clone();
        bad.packages[0].id = "different-id".into();
        rejected(VALID, &bad, "member has no package record");
        let mut bad = good.clone();
        bad.packages.push(good.packages[0].clone());
        rejected(VALID, &bad, "duplicate package IDs");
        let mut bad = good;
        bad.packages[0].manifest_path = bad.workspace_root.join("crates/alpha/not-Cargo.toml");
        rejected(VALID, &bad, "manifest must be root-relative Cargo.toml");
    }

    #[test]
    fn deterministic_errors_do_not_depend_on_cargo_package_order() {
        let mut inventory = inventory();
        inventory.packages.push(CargoPackage {
            id: "beta-id".into(),
            name: "beta".into(),
            version: "0.0.0".into(),
            manifest_path: inventory.workspace_root.join("crates/beta/Cargo.toml"),
            source: None,
            dependencies: Vec::new(),
        });
        inventory.workspace_members.push("beta-id".into());
        accepted(&format!("{VALID}{EXTRA_ROW}"), &inventory);
        let first = validate_status_text(VALID, &inventory);
        inventory.packages.reverse();
        inventory.workspace_members.reverse();
        assert_eq!(first, validate_status_text(VALID, &inventory));
        assert_eq!(first, vec!["PACKAGE_STATUS: missing package row beta"]);
    }

    #[test]
    fn reference_and_text_limits_are_inclusive_and_overflow_is_terminal() {
        let at_limit = format!("consumers = [{}]", vec!["\"#251\""; MAX_ITEMS].join(","));
        let text = VALID.replacen("consumers = [\"#251\"]", &at_limit, 1);
        accepted(&text, &inventory());
        let over_limit = format!(
            "consumers = [{}]",
            vec!["\"#251\""; MAX_ITEMS + 1].join(",")
        );
        mutation(
            &text,
            &at_limit,
            &over_limit,
            "consumers must contain bounded references",
        );
        let exact = format!("{VALID}#{}", "x".repeat(MAX_STATUS_BYTES - VALID.len() - 1));
        assert_eq!(exact.len(), MAX_STATUS_BYTES);
        accepted(&exact, &inventory());
        let over = format!("{exact}x");
        rejected(&over, &inventory(), "status text exceeds one MiB");
    }

    #[test]
    fn read_cap_checks_before_growth_and_read_failure_is_terminal() {
        struct FailedReader;
        impl Read for FailedReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fixture read failure"))
            }
        }
        let exact = std::io::repeat(b'x').take(MAX_READ_BYTES - 1);
        assert_eq!(
            read_capped(exact, "status file")
                .expect("inclusive cap")
                .len(),
            MAX_STATUS_BYTES
        );
        assert!(
            read_capped(std::io::repeat(b'x'), "status file")
                .expect_err("overflow sentinel")
                .contains("exceeds one MiB")
        );
        assert!(
            read_capped(FailedReader, "status file")
                .expect_err("read failure")
                .contains("cannot read status file")
        );
    }

    #[test]
    fn diagnostic_count_bytes_and_utf8_are_bounded_and_stable() {
        let bad_references = format!("consumers = [{}]", vec!["\"bad\""; MAX_ITEMS].join(","));
        let text = VALID.replacen("consumers = [\"#251\"]", &bad_references, 1);
        accepted(VALID, &inventory());
        let errors = validate_status_text(&text, &inventory());
        assert_eq!(errors.len(), MAX_ERRORS);
        assert!(errors.iter().all(|error| error.len() <= MAX_ERROR_BYTES));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("additional validation errors omitted"))
        );
        assert!(errors.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(errors, validate_status_text(&text, &inventory()));
        let mut diagnostics = Diagnostics::default();
        diagnostics.push("é".repeat(MAX_ERROR_BYTES));
        let bounded = diagnostics.finish();
        assert_eq!(bounded[0].len(), MAX_ERROR_BYTES);
        assert!(std::str::from_utf8(bounded[0].as_bytes()).is_ok());
    }

    struct ReadmeFixture {
        root: std::path::PathBuf,
        inventory: CargoInventory,
    }

    impl ReadmeFixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "eliot-status-readme-{}-{sequence}",
                std::process::id()
            ));
            // Create a fresh directory before taking cleanup ownership.
            std::fs::create_dir(&root).expect("fresh fixture directory");
            let mut inventory = inventory();
            inventory.workspace_root.clone_from(&root);
            inventory.packages[0].manifest_path = root.join("crates/alpha/Cargo.toml");
            let fixture = Self { root, inventory };
            std::fs::create_dir_all(fixture.root.join("docs/product")).expect("status directory");
            std::fs::create_dir_all(fixture.root.join("crates/alpha")).expect("package directory");
            std::fs::write(fixture.root.join(STATUS_FILE), VALID).expect("valid status");
            fixture
        }

        fn readme(&self) -> std::path::PathBuf {
            self.root.join("crates/alpha/README.md")
        }
    }

    impl Drop for ReadmeFixture {
        fn drop(&mut self) {
            let (Ok(root), Ok(temporary)) = (
                std::fs::canonicalize(&self.root),
                std::fs::canonicalize(std::env::temp_dir()),
            ) else {
                return;
            };
            // Cleanup only the fresh, owned child of the resolved temp directory.
            if root.parent() == Some(temporary.as_path()) {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
    }

    #[test]
    fn readme_phrase_tokens_handle_line_breaks_case_and_markdown() {
        for text in [
            "intentionally unimplemented.",
            "intentionally\n\nunimplemented",
            "INTENTIONALLY\r\n\tUNIMPLEMENTED.",
            "**intentionally**\u{00a0}`unimplemented`",
        ] {
            assert!(has_banned_readme_status(text), "missed phrase: {text:?}");
        }
        for text in [
            "unimplemented intentionally",
            "intentionally implemented",
            "intentionally still unimplemented",
            "unintentionally unimplemented",
            "intentionally unimplementedness",
        ] {
            assert!(!has_banned_readme_status(text), "false phrase: {text:?}");
        }
    }

    #[test]
    fn readme_scan_skips_missing_and_reports_disabled_member_violation() {
        let fixture = ReadmeFixture::new();
        let disabled = VALID.replacen(
            "disposition = \"active\"",
            "disposition = \"evaluation_only\"",
            1,
        );
        std::fs::write(fixture.root.join(STATUS_FILE), disabled).expect("valid evaluation status");
        assert!(validate_package_status(&fixture.root, &fixture.inventory).is_empty());
        std::fs::write(fixture.readme(), "Status: intentionally\n unimplemented.\n")
            .expect("one README fault");
        let errors = validate_package_status(&fixture.root, &fixture.inventory);
        assert_eq!(
            errors,
            vec![
                "PACKAGE_STATUS: crates/alpha/README.md (alpha) contains banned phrase 'intentionally unimplemented'"
            ]
        );
    }

    #[test]
    fn readme_utf8_size_and_nonregular_errors_are_terminal() {
        let fixture = ReadmeFixture::new();
        assert!(validate_package_status(&fixture.root, &fixture.inventory).is_empty());
        std::fs::write(fixture.readme(), [0xff]).expect("one UTF-8 fault");
        assert!(
            validate_package_status(&fixture.root, &fixture.inventory)
                .iter()
                .any(|error| error.contains("README must be UTF-8"))
        );
        std::fs::write(fixture.readme(), vec![b'a'; MAX_STATUS_BYTES])
            .expect("inclusive README cap");
        assert!(validate_package_status(&fixture.root, &fixture.inventory).is_empty());
        std::fs::write(fixture.readme(), vec![b'a'; MAX_STATUS_BYTES + 1]).expect("one size fault");
        assert!(
            validate_package_status(&fixture.root, &fixture.inventory)
                .iter()
                .any(|error| error.contains("README exceeds one MiB"))
        );
        std::fs::remove_file(fixture.readme()).expect("remove owned fixture file");
        std::fs::create_dir(fixture.readme()).expect("one nonregular-file fault");
        let errors = validate_package_status(&fixture.root, &fixture.inventory);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("cannot open README")
                    || error.contains("README input must be a regular file")),
            "nonregular README must fail: {errors:?}"
        );
    }

    #[test]
    fn nonmember_package_readmes_are_excluded_from_scan() {
        let mut fixture = ReadmeFixture::new();
        assert!(validate_package_status(&fixture.root, &fixture.inventory).is_empty());
        let external = fixture.root.join("crates/beta");
        std::fs::create_dir(&external).expect("nonmember directory");
        std::fs::write(external.join("README.md"), "intentionally unimplemented")
            .expect("nonmember README");
        fixture.inventory.packages.push(CargoPackage {
            id: "beta-id".into(),
            name: "beta".into(),
            version: "0.0.0".into(),
            manifest_path: external.join("Cargo.toml"),
            source: None,
            dependencies: Vec::new(),
        });
        assert!(validate_package_status(&fixture.root, &fixture.inventory).is_empty());
    }
}
