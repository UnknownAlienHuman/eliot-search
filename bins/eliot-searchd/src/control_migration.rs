//! Read-only, bounded source-history input for legacy control migration.
//!
//! The running service already holds the data-root owner. This operation creates
//! no DirectStore, redb, source event, preparation object, or recovery receipt.
//! A page is returned only after replay of the complete source chain succeeds.
//! Root registration, directory manifests, payload verification and canonical
//! H5 mapping are separate migration inputs, not implied by this source page.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::{
    CONTROL_DIRECTORY, DirectStore, MAX_SOURCE_EVENTS, NAMESPACE_FILE, Path,
    SOURCE_LOG_FILE, SourceRecord, SourceState, ZERO_DIGEST, read_namespace,
    replay_registry, sha256,
};

const PAGE_EVENTS: usize = 32;
const MAX_PAGE_BYTES: usize = 60 * 1024;
const REPLAY_DEADLINE: Duration = Duration::from_secs(30);

/// Administrative bookmark, not authentication or proof of a processed prefix.
struct Cursor {
    snapshot: [u8; 32],
    after: u64,
    event_digest: String,
}

impl Cursor {
    fn parse(value: &str) -> Result<Self, String> {
        if value.len() > 153 || !value.is_ascii() {
            return Err("DIRECT_MIGRATION_CURSOR_INVALID".to_owned());
        }
        let fields = value.splitn(5, '.').collect::<Vec<_>>();
        if fields.len() != 4 || fields[0] != "m1" {
            return Err("DIRECT_MIGRATION_CURSOR_INVALID".to_owned());
        }
        let snapshot = sha256::decode_digest(fields[1])
            .ok_or_else(|| "DIRECT_MIGRATION_CURSOR_INVALID".to_owned())?;
        let after = fields[2].parse::<u64>()
            .map_err(|_| "DIRECT_MIGRATION_CURSOR_INVALID".to_owned())?;
        let event_digest = sha256::decode_digest(fields[3])
            .ok_or_else(|| "DIRECT_MIGRATION_CURSOR_INVALID".to_owned())?;
        if after == 0 || after > MAX_SOURCE_EVENTS as u64
            || after.to_string() != fields[2]
            || sha256::hex(&snapshot) != fields[1]
            || sha256::hex(&event_digest) != fields[3]
        {
            return Err("DIRECT_MIGRATION_CURSOR_INVALID".to_owned());
        }
        Ok(Self { snapshot, after, event_digest: fields[3].to_owned() })
    }
}

impl DirectStore {
    /// Verify the migration-grade source chain against this owner's admitted
    /// state. Object pages use the same logical snapshot identity as event pages.
    /// The supplied deadline spans both replays and all intervening object I/O.
    pub(crate) fn verify_migration_snapshot(&self, deadline: Instant) -> Result<[u8; 32], String> {
        if Instant::now() >= deadline {
            return Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned());
        }
        let control = self.root.join(CONTROL_DIRECTORY);
        let namespace = read_namespace(&control.join(NAMESPACE_FILE))?;
        if namespace != self.namespace_id {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        let state = replay_registry(&control.join(SOURCE_LOG_FILE), |record, previous| {
            if Instant::now() >= deadline {
                return Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned());
            }
            validate_legacy_event(&namespace, record, previous)
        })?;
        if state != self.registry || read_namespace(&control.join(NAMESPACE_FILE))? != namespace {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        if Instant::now() >= deadline {
            return Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned());
        }
        Ok(snapshot_digest(&namespace, state.last_sequence, &state.last_digest))
    }

    /// Inspect disk history under the caller's existing exclusive owner guard.
    /// `expected_namespace` is taken from the admitted live store, never a client.
    /// Returned JSON is one bounded event-first frame for the existing transport.
    /// A malformed suffix or stale cursor produces no acknowledged partial page.
    pub(crate) fn inspect_control_history(
        root: &Path,
        expected_namespace: &str,
        cursor: Option<&str>,
    ) -> Result<String, String> {
        let started = Instant::now();
        let cursor = cursor.map(Cursor::parse).transpose()?;
        let control = root.join(CONTROL_DIRECTORY);
        let namespace = read_namespace(&control.join(NAMESPACE_FILE))?;
        let namespace_text = sha256::hex(&namespace);
        if namespace_text != expected_namespace {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        let after = cursor.as_ref().map_or(0, |value| value.after);
        let mut cursor_found = cursor.is_none();
        let mut ordinals = BTreeMap::<String, u64>::new();
        let mut entries = Vec::with_capacity(PAGE_EVENTS);
        let mut last = after;
        let mut last_digest = cursor.as_ref().map_or(ZERO_DIGEST, |value| value.event_digest.as_str()).to_owned();
        let state = replay_registry(&control.join(SOURCE_LOG_FILE), |record, previous| {
            if started.elapsed() >= REPLAY_DEADLINE {
                return Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned());
            }
            validate_legacy_event(&namespace, record, previous)?;
            // This ordinal counts this source's events, including retirement.
            // It is deliberately NOT a SourceRevision or a content-object ID.
            let ordinal = ordinals.entry(record.source_id.clone()).or_default();
            *ordinal = ordinal.checked_add(1)
                .ok_or_else(|| "DIRECT_MIGRATION_ORDINAL_EXHAUSTED".to_owned())?;
            if record.sequence == after {
                cursor_found = record.record_digest == last_digest;
            }
            if record.sequence > after && entries.len() < PAGE_EVENTS {
                entries.push(event_json(record, previous, *ordinal));
                last = record.sequence;
                last_digest.clone_from(&record.record_digest);
            }
            Ok(())
        })?;
        if started.elapsed() >= REPLAY_DEADLINE {
            return Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned());
        }
        if read_namespace(&control.join(NAMESPACE_FILE))? != namespace {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        // The replay validator also checks the same opened file's length and
        // metadata before/after reading. This digest denotes its logical chain,
        // not a byte-for-byte file hash (LF/CRLF framing is not conflated with it).
        let snapshot = snapshot_digest(&namespace, state.last_sequence, &state.last_digest);
        if !cursor_found || cursor.as_ref().is_some_and(|value| value.snapshot != snapshot) {
            return Err("DIRECT_MIGRATION_CURSOR_STALE".to_owned());
        }
        let body = entries.join(",");
        let page_digest = sha256::digest_parts(b"eliot-search/control-migration-page/v1", &[
            &snapshot, &after.to_be_bytes(), &last.to_be_bytes(), body.as_bytes(),
        ]);
        let exhausted = last == state.last_sequence;
        let next_cursor = if exhausted {
            "null".to_owned()
        } else {
            format!("\"m1.{}.{last}.{last_digest}\"", sha256::hex(&snapshot))
        };
        // All interpolated strings are validated hex, closed tags or generated
        // JSON. No source bodies, path text, credentials or client strings enter it.
        let output = format!(
            concat!(
                "{{\"event\":\"control_migration_page\",",
                "\"schema\":\"legacy-source-history-v1\",\"scope\":\"source_events_only\",",
                "\"namespace_id\":\"{}\",\"snapshot_sha256\":\"{}\",",
                "\"source_chain_sha256\":\"{}\",\"source_events\":{},",
                "\"sources\":{},\"referenced_revision_ids\":{},",
                "\"after_sequence\":{},\"last_sequence\":{},\"page_events\":{},",
                "\"entries\":[{}],\"page_sha256\":\"{}\",",
                "\"next_cursor\":{},\"exhausted\":{},\"read_only\":true,",
                "\"revision_payloads_verified\":false}}"
            ),
            namespace_text, sha256::hex(&snapshot), state.last_digest, state.event_count,
            state.latest.len(), state.revisions.len(), after, last, entries.len(),
            body, sha256::hex(&page_digest), next_cursor, exhausted,
        );
        if output.len() > MAX_PAGE_BYTES {
            return Err("DIRECT_MIGRATION_PAGE_TOO_LARGE".to_owned());
        }
        Ok(output)
    }
}

fn snapshot_digest(namespace: &[u8; 32], sequence: u64, last_digest: &str) -> [u8; 32] {
    sha256::digest_parts(b"eliot-search/control-migration-snapshot/v1", &[
        namespace, &sequence.to_be_bytes(), last_digest.as_bytes(),
    ])
}

fn validate_legacy_event(
    namespace: &[u8; 32], record: &SourceRecord, previous: Option<&SourceRecord>,
) -> Result<(), String> {
    let file_identity = sha256::decode_digest(&record.file_identity_digest)
        .ok_or_else(|| "DIRECT_MIGRATION_SOURCE_BINDING_INVALID".to_owned())?;
    let expected = sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-source-id/v1", &[namespace, &file_identity],
    ));
    if record.source_id != expected
        || previous.is_some_and(|value| value.identity_strength != record.identity_strength)
    {
        return Err("DIRECT_MIGRATION_SOURCE_BINDING_INVALID".to_owned());
    }
    // Never silently normalize a hand-edited legacy record whose hash covered
    // a different spelling. The actual legacy writer emits this exact encoding.
    if sha256::hex(&sha256::digest(record.canonical_without_digest().as_bytes())) != record.record_digest {
        return Err("DIRECT_MIGRATION_LEGACY_ENCODING_UNSUPPORTED".to_owned());
    }
    if record.state == SourceState::Retired {
        let previous = previous.ok_or_else(|| "DIRECT_MIGRATION_RETIREMENT_INVALID".to_owned())?;
        if previous.state != SourceState::Active
            || previous.revision_id != record.revision_id
            || previous.content_digest != record.content_digest
            || previous.byte_length != record.byte_length
            || previous.path_digest != record.path_digest
        {
            return Err("DIRECT_MIGRATION_RETIREMENT_INVALID".to_owned());
        }
    }
    Ok(())
}

fn event_json(record: &SourceRecord, previous: Option<&SourceRecord>, ordinal: u64) -> String {
    format!(
        concat!(
            "{{\"event_sequence\":{},\"source_event_ordinal\":{},",
            "\"previous_event_sha256\":\"{}\",\"previous_source_event_sha256\":\"{}\",",
            "\"operation_id\":\"{}\",\"state\":\"{}\",\"source_id\":\"{}\",",
            "\"legacy_revision_id\":\"{}\",\"content_sha256\":\"{}\",\"byte_length\":{},",
            "\"file_identity_sha256\":\"{}\",\"path_sha256\":\"{}\",",
            "\"identity_strength\":\"{}\",\"event_sha256\":\"{}\"}}"
        ),
        record.sequence, ordinal, record.previous_digest,
        previous.map_or(ZERO_DIGEST, |value| value.record_digest.as_str()),
        record.operation_id, record.state.tag(), record.source_id,
        record.revision_id, record.content_digest, record.byte_length,
        record.file_identity_digest, record.path_digest, record.identity_strength.tag(), record.record_digest,
    )
}
