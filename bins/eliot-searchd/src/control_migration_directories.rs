//! Root registration and immutable directory history under the current DIRECT owner.
//! Only the selected page's entries are bound to source events; no import is implied.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use super::{DirectStore, DEADLINE, MAX_PAGE_BYTES, check_deadline};
use crate::development::DataRootGuard;
use crate::directory_manifest::{migration_manifest, migration_manifest_files, path_identity_bytes};
use crate::service_output::json_string;
use crate::{sha256, source_roots};

const PAGE_ROWS: usize = 32;
const MAX_FILES: usize = 65_536;
const MAX_INVENTORY_BYTES: usize = 256 * 1024 * 1024;

struct Cursor {
    snapshot: [u8; 32],
    after: u64,
}
impl Cursor {
    fn parse(value: &str) -> Result<Self, String> {
        let invalid = || "DIRECT_MIGRATION_DIRECTORY_CURSOR_INVALID".to_owned();
        if value.len() > 88 || !value.is_ascii() { return Err(invalid()); }
        let parts = value.splitn(4, '.').collect::<Vec<_>>();
        if parts.len() != 3 || parts[0] != "d1" { return Err(invalid()); }
        let snapshot = sha256::decode_digest(parts[1]).ok_or_else(invalid)?;
        let after = parts[2].parse::<u64>().map_err(|_| invalid())?;
        if sha256::hex(&snapshot) != parts[1] || after.to_string() != parts[2] {
            return Err(invalid());
        }
        Ok(Self { snapshot, after })
    }
}

#[derive(Eq, PartialEq)]
struct Inventory {
    directory_present: bool,
    file_count: usize,
    directory_count: usize,
    encoded_bytes: usize,
    rows: u64,
    digest: [u8; 32],
}

struct Page {
    after: u64,
    rows: Vec<String>,
    // Exact legacy source/revision/path tuples, bounded by PAGE_ROWS.
    bindings: Vec<(String, String, String)>,
}
impl Page {
    const fn wants(&self, ordinal: u64) -> bool {
        ordinal >= self.after && self.rows.len() < PAGE_ROWS
    }
}

impl DirectStore {
    /// Emits registration plus a bounded page from every extant manifest generation.
    /// Both complete metadata sweeps and source replays share one cooperative deadline.
    /// Startup migration is not made side-effect-free by this already-open-store read.
    /// The borrowed live owner binds the root and its admitted registration throughout.
    pub(crate) fn inspect_migration_directories(
        &self, owner: &DataRootGuard, cursor: Option<&str>,
    ) -> Result<String, String> {
        let deadline = Instant::now().checked_add(DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let cursor = cursor.map(Cursor::parse).transpose()?;
        if owner.canonical_root() != self.root.as_path() {
            return Err("DIRECT_MIGRATION_ROOT_OWNER_MISMATCH".to_owned());
        }
        let admitted = owner.source_roots().views().map_err(|error| error.code().to_owned())?;
        let catalog = self.inner.verify_migration_snapshot(deadline)?;
        let roots = source_roots::migration_input(&self.root).map_err(|error| error.code().to_owned())?;
        // A valid replacement or deleted registration file is not a new admitted
        // root set. Compare locators only: cached availability is not live authority.
        if !roots.paths.iter().map(std::path::PathBuf::as_path).eq(
            admitted.iter().map(|view| std::path::Path::new(&view.path)),
        ) {
            return Err("DIRECT_MIGRATION_ROOT_STATE_CHANGED".to_owned());
        }
        let root_ids = roots.paths.iter().map(|path| {
            let bytes = path_identity_bytes(path);
            (sha256::hex(&sha256::digest(&bytes)), sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-directory/v1", &[&bytes],
            )))
        }).collect::<Vec<_>>();
        let roots_json = root_ids.iter().enumerate().map(|(index, (path, directory))| format!(
            "{{\"root_index\":{index},\"native_path_sha256\":\"{path}\",\"directory_sha256\":\"{directory}\"}}",
        )).collect::<Vec<_>>().join(",");
        let root_digest = roots.file_bytes.as_deref().map(sha256::digest);
        let mut page = Page {
            after: cursor.as_ref().map_or(0, |value| value.after),
            rows: Vec::with_capacity(PAGE_ROWS), bindings: Vec::with_capacity(PAGE_ROWS),
        };
        let before = self.directory_inventory(&root_ids, Some(&mut page), deadline)?;
        let snapshot = sha256::digest_parts(b"eliot-search/control-migration-directories/v1", &[
            &catalog, &[u8::from(root_digest.is_some())], &root_digest.unwrap_or([0; 32]),
            &[u8::from(before.directory_present)], &before.digest,
            &(before.file_count as u64).to_be_bytes(), &before.rows.to_be_bytes(),
        ]);
        if page.after > before.rows || cursor.as_ref().is_some_and(|value| value.snapshot != snapshot) {
            return Err("DIRECT_MIGRATION_DIRECTORY_CURSOR_STALE".to_owned());
        }
        let after = self.directory_inventory(&root_ids, None, deadline)?;
        if before != after || source_roots::migration_input(&self.root)
            .map_err(|error| error.code().to_owned())? != roots
        {
            return Err("DIRECT_MIGRATION_METADATA_CHANGED".to_owned());
        }
        // Match historical tuples, not just a source's current path or the first
        // retained object occurrence. Renames and A/B/A transitions remain distinct.
        if self.inner.verify_migration_bindings(&page.bindings, deadline)? != catalog {
            return Err("DIRECT_MIGRATION_DIRECTORY_CURSOR_STALE".to_owned());
        }
        let next = page.after + page.rows.len() as u64; // bounded by verified inventory rows
        let exhausted = next == before.rows;
        let body = page.rows.join(",");
        let page_digest = sha256::digest_parts(b"eliot-search/control-migration-directory-page/v1", &[
            &snapshot, &page.after.to_be_bytes(), &next.to_be_bytes(),
            roots_json.as_bytes(), body.as_bytes(),
        ]);
        let next_cursor = if exhausted { "null".to_owned() }
            else { json_string(&format!("d1.{}.{next}", sha256::hex(&snapshot))) };
        let output = format!(
            concat!(
                "{{\"event\":\"control_migration_directories\",\"schema\":\"legacy-directory-input-v1\",",
                "\"namespace_id\":{},\"catalog_snapshot_sha256\":\"{}\",\"snapshot_sha256\":\"{}\",",
                "\"root_file_present\":{},\"root_file_bytes\":{},\"root_file_sha256\":{},",
                "\"registered_roots\":[{}],\"manifest_directory_present\":{},",
                "\"manifest_files\":{},\"directories\":{},\"manifest_bytes\":{},",
                "\"inventory_rows\":{},\"after_row\":{},\"next_row\":{},\"page_rows\":{},\"entries\":[{}],",
                "\"page_sha256\":\"{}\",\"next_cursor\":{},\"exhausted\":{},",
                "\"read_only\":true,\"page_history_bindings_verified\":true,",
                "\"root_locators_exported\":false,\"live_root_state_checked\":false,",
                "\"payloads_verified\":false,\"canonical_mapping_complete\":false}}"
            ),
            json_string(&self.namespace_id()), sha256::hex(&catalog), sha256::hex(&snapshot),
            root_digest.is_some(), roots.file_bytes.as_ref().map_or(0, Vec::len),
            root_digest.map_or_else(|| "null".to_owned(), |value| json_string(&sha256::hex(&value))),
            roots_json, before.directory_present, before.file_count, before.directory_count,
            before.encoded_bytes, before.rows, page.after, next, page.rows.len(), body,
            sha256::hex(&page_digest), next_cursor, exhausted,
        );
        if output.len() > MAX_PAGE_BYTES { return Err("DIRECT_MIGRATION_PAGE_TOO_LARGE".to_owned()); }
        check_deadline(Some(deadline))?;
        Ok(output)
    }

    fn directory_inventory(
        &self, roots: &[(String, String)], mut page: Option<&mut Page>, deadline: Instant,
    ) -> Result<Inventory, String> {
        let (directory_present, files) = migration_manifest_files(&self.root, MAX_FILES, deadline)?;
        let namespace = self.namespace_id();
        let mut generations = BTreeMap::<String, BTreeSet<u64>>::new();
        let mut result = Inventory {
            directory_present, file_count: files.len(), directory_count: 0,
            encoded_bytes: 0, rows: 0,
            digest: sha256::digest_parts(b"eliot-search/directory-inventory-seed/v1", &[]),
        };
        for path in &files {
            check_deadline(Some(deadline))?;
            let remaining = MAX_INVENTORY_BYTES.checked_sub(result.encoded_bytes)
                .ok_or_else(|| "DIRECT_MIGRATION_METADATA_BYTES_EXCEEDED".to_owned())?;
            let (manifest, raw_digest, bytes) = migration_manifest(path, remaining)?;
            if manifest.namespace_id != namespace {
                return Err("DIRECT_MANIFEST_NAMESPACE_MISMATCH".to_owned());
            }
            if !generations.entry(manifest.directory_digest.clone()).or_default().insert(manifest.generation) {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
            let name = path.file_name().and_then(|name| name.to_str())
                .ok_or_else(|| "DIRECT_MANIFEST_FILENAME_INVALID".to_owned())?;
            result.digest = sha256::digest_parts(b"eliot-search/directory-inventory-step/v1", &[
                &result.digest, name.as_bytes(), &(bytes as u64).to_be_bytes(), &raw_digest,
            ]);
            result.encoded_bytes += bytes; // read was limited to remaining bytes
            if let Some(selected) = page.as_deref_mut().filter(|selected| selected.wants(result.rows)) {
                let root = roots.iter().position(|(_, digest)| digest == &manifest.directory_digest)
                    .map_or_else(|| "null".to_owned(), |index| index.to_string());
                selected.rows.push(format!(
                    concat!(
                        "{{\"kind\":\"manifest\",\"directory_sha256\":{},\"generation\":{},",
                        "\"manifest_sha256\":{},\"file_sha256\":\"{}\",\"encoded_bytes\":{},",
                        "\"entry_count\":{},\"exact_registered_root_index\":{}}}"
                    ),
                    json_string(&manifest.directory_digest), manifest.generation,
                    json_string(&manifest.manifest_digest), sha256::hex(&raw_digest),
                    bytes, manifest.entries.len(), root,
                ));
            }
            result.rows = result.rows.checked_add(1)
                .ok_or_else(|| "DIRECT_MIGRATION_METADATA_COUNT_EXCEEDED".to_owned())?;
            for entry in manifest.entries.values() {
                check_deadline(Some(deadline))?;
                if let Some(selected) = page.as_deref_mut().filter(|selected| selected.wants(result.rows)) {
                    selected.rows.push(format!(
                        concat!(
                            "{{\"kind\":\"entry\",\"directory_sha256\":{},\"generation\":{},",
                            "\"manifest_sha256\":{},\"source_id\":{},\"legacy_revision_id\":{},\"path_sha256\":{}}}"
                        ),
                        json_string(&manifest.directory_digest), manifest.generation,
                        json_string(&manifest.manifest_digest), json_string(&entry.source_id),
                        json_string(&entry.revision_id), json_string(&entry.path_digest),
                    ));
                    selected.bindings.push((entry.source_id.clone(), entry.revision_id.clone(), entry.path_digest.clone()));
                }
                result.rows = result.rows.checked_add(1)
                    .ok_or_else(|| "DIRECT_MIGRATION_METADATA_COUNT_EXCEEDED".to_owned())?;
            }
        }
        // Generation numbers start at one. A unique set with max != count has
        // a missing earlier generation. Loss of an entire directory/final suffix
        // still needs an independent inventory anchor; this view cannot invent it.
        for values in generations.values() {
            if values.last().copied() != Some(values.len() as u64) {
                return Err("DIRECT_MIGRATION_MANIFEST_HISTORY_GAP".to_owned());
            }
        }
        result.directory_count = generations.len();
        check_deadline(Some(deadline))?;
        Ok(result)
    }
}

