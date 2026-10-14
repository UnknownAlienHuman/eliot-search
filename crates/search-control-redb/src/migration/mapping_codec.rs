//! Strict canonical source-mapping v1 rows, shared by producer and consumers.
//! Decoding checks syntax and the existing pure mapper; source provenance remains
//! with the verified original-artifact capability, not this constructible decoder.

use search_contracts::Sha256Digest32;
use super::{LegacySourceMappingEvent, LegacySourceMappingState, MappedSourceEvent,
    SourceImportRow, SourceMappingError as Error, SourceMappingHeader,
    SourceMappingPlanner, SourceMappingSummary, MAX_SOURCE_IMPORT_ROW_BYTES,
    source_mapping_profile_digest};

/// Exact canonical header bytes of the retained v1 format.
#[must_use]
pub fn encode_source_mapping_header(header: SourceMappingHeader) -> Vec<u8> {
    format!(concat!(
        "{{\"kind\":\"source_mapping_header\",\"schema\":\"eliot.source-mapping.v1\",",
        "\"target_namespace_id\":\"{}\",\"legacy_namespace_sha256\":\"{}\",",
        "\"catalog_snapshot_sha256\":\"{}\",\"mapping_profile_sha256\":\"{}\",",
        "\"expected_events\":{},\"expected_sources\":{},\"identity_kind\":\"imported_object\",",
        "\"acquisition_kind\":\"imported\",\"draft_only\":true,\"cutover_authorized\":false}}\n"),
        header.namespace, Sha256Digest32::from_stored_bytes(header.legacy_namespace),
        Sha256Digest32::from_stored_bytes(header.catalog_snapshot),
        Sha256Digest32::from_stored_bytes(source_mapping_profile_digest()),
        header.expected_events, header.expected_sources).into_bytes()
}

/// Exact canonical mapped-event bytes of the retained v1 format.
#[must_use]
pub fn encode_source_mapping_event(mapped: &MappedSourceEvent, event: &LegacySourceMappingEvent) -> Vec<u8> {
    let predecessor = mapped.previous_revision_id.map_or_else(|| "null".to_owned(), |id| format!("\"{id}\""));
    let legacy = format!(concat!(
        "{{\"event_sequence\":{},\"source_event_ordinal\":{},\"previous_event_sha256\":\"{}\",",
        "\"previous_source_event_sha256\":\"{}\",\"operation_id\":\"{}\",\"state\":\"{}\",",
        "\"source_id\":\"{}\",\"legacy_revision_id\":\"{}\",\"content_sha256\":\"{}\",\"byte_length\":{},",
        "\"file_identity_sha256\":\"{}\",\"path_sha256\":\"{}\",\"identity_strength\":\"{}\",\"event_sha256\":\"{}\"}}"),
        event.sequence, mapped.source_event_ordinal, event.previous_event,
        event.previous_source_event.unwrap_or(Sha256Digest32::from_stored_bytes([0; 32])),
        event.operation, match event.state { LegacySourceMappingState::Active => "A", LegacySourceMappingState::Retired => "R" },
        event.legacy_source, event.legacy_revision, event.content, event.source_bytes,
        event.file_identity, event.path, if event.native_identity { "native" } else { "path-bound" }, event.event);
    format!(concat!(
        "{{\"kind\":\"source_event_mapping\",\"source_id\":\"{}\",\"revision_id\":\"{}\",",
        "\"occurrence_sequence\":{},\"previous_revision_id\":{},\"opens_source\":{},",
        "\"opens_revision\":{},\"retires_source\":{},\"legacy\":{}}}\n"),
        mapped.source_id, mapped.revision_id, mapped.occurrence_sequence,
        predecessor, mapped.opens_source, mapped.opens_revision, mapped.retires_source, legacy).into_bytes()
}

/// Exact canonical footer bytes of the retained v1 format.
#[must_use]
pub fn encode_source_mapping_summary(summary: SourceMappingSummary) -> Vec<u8> {
    format!(concat!(
        "{{\"kind\":\"source_mapping_end\",\"events\":{},\"sources\":{},\"revision_occurrences\":{},",
        "\"retained_revision_events\":{},\"retirements\":{},\"required_import_inputs\":[",
        "\"content_blake3_readback\",\"import_observation_and_stability\",\"residency_policy\",",
        "\"root_and_membership_bindings\",\"namespace_owner_cutover\"],\"canonical_records_materialized\":false}}\n"),
        summary.events, summary.sources, summary.occurrences,
        summary.path_only_events, summary.retirements).into_bytes()
}

/// One strictly decoded source-mapping record.
#[derive(Clone, Debug, Eq, PartialEq)]
#[expect(clippy::large_enum_variant, reason = "The finite fixed-size import row stays on the stack; boxing would add a heap allocation per event.")]
pub enum SourceMappingDecodedRow {
    /// Exact expected header.
    Header,
    /// Typed row derived by the existing mapper, not supplied imported IDs.
    Event(SourceImportRow),
    /// Recomputed exact accounting.
    End(SourceMappingSummary),
}

/// Finite streaming decoder; refusal remains sticky for this complete pass.
pub struct SourceMappingManifestDecoder {
    header: Vec<u8>,
    planner: Option<SourceMappingPlanner>,
    started: bool,
    summary: Option<SourceMappingSummary>,
    previous_event: Sha256Digest32,
    refused: Option<Error>,
}

impl SourceMappingManifestDecoder {
    /// Binds a pass to the expected header and the existing mapper's finite bound.
    /// # Errors
    /// Invalid namespace, cardinality or limits refuse before allocation.
    pub fn new(header: SourceMappingHeader, max_events: usize) -> Result<Self, Error> {
        let header = SourceMappingHeader::new(header.namespace, header.legacy_namespace,
            header.catalog_snapshot, header.expected_events, header.expected_sources,
            u64::try_from(max_events).map_err(|_| Error::Limit)?)?;
        Ok(Self { header: encode_source_mapping_header(header),
            planner: Some(SourceMappingPlanner::new(header, max_events)?),
            started: false, summary: None,
            previous_event: Sha256Digest32::from_stored_bytes([0; 32]), refused: None })
    }

    /// Decodes one exact bounded row through the shared encoder and mapper.
    /// # Errors
    /// Missing/duplicate/unknown fields, noncanonical bytes, identity or sequence
    /// mismatch, and surplus rows refuse this decoder permanently.
    pub fn push(&mut self, row: &[u8]) -> Result<SourceMappingDecodedRow, Error> {
        if let Some(error) = self.refused { return Err(error); }
        let result = self.decode(row);
        if let Err(error) = result { self.refused = Some(error); }
        result
    }

    fn decode(&mut self, row: &[u8]) -> Result<SourceMappingDecodedRow, Error> {
        if row.len() > MAX_SOURCE_IMPORT_ROW_BYTES { return Err(Error::Limit); }
        let line = row.strip_suffix(b"\n").ok_or(Error::OrderInvalid)?;
        if line.contains(&b'\n') { return Err(Error::OrderInvalid); }
        if !self.started {
            if row != self.header { return Err(Error::SummaryMismatch); }
            self.started = true;
            return Ok(SourceMappingDecodedRow::Header);
        }
        let text = core::str::from_utf8(line).map_err(|_| Error::OrderInvalid)?;
        let planner = self.planner.as_mut().ok_or(Error::OrderInvalid)?;
        if text.starts_with("{\"kind\":\"source_mapping_end\",") {
            let summary = self.planner.take().ok_or(Error::OrderInvalid)?.finish()?;
            if row != encode_source_mapping_summary(summary) { return Err(Error::SummaryMismatch); }
            self.summary = Some(summary);
            return Ok(SourceMappingDecodedRow::End(summary));
        }
        if !text.starts_with("{\"kind\":\"source_event_mapping\",") { return Err(Error::OrderInvalid); }
        let (_, legacy) = text.split_once(",\"legacy\":").ok_or(Error::OrderInvalid)?;
        let (event, ordinal) = parse_legacy(legacy.strip_suffix('}').ok_or(Error::OrderInvalid)?)?;
        if event.previous_event != self.previous_event { return Err(Error::PredecessorInvalid); }
        let mapped = planner.map(event)?;
        if ordinal != mapped.source_event_ordinal || row != encode_source_mapping_event(&mapped, &event) {
            return Err(Error::SummaryMismatch);
        }
        self.previous_event = event.event;
        Ok(SourceMappingDecodedRow::Event(mapped.import_row()))
    }

    /// Completes only after the exact footer and without any prior refusal.
    /// # Errors
    /// Incomplete streams and sticky failures never produce accounting success.
    pub fn finish(self) -> Result<SourceMappingSummary, Error> {
        if let Some(error) = self.refused { return Err(error); }
        self.summary.ok_or(Error::SummaryMismatch)
    }
}

fn parse_legacy(text: &str) -> Result<(LegacySourceMappingEvent, u64), Error> {
    let mut fields = text.strip_prefix('{').and_then(|value| value.strip_suffix('}'))
        .ok_or(Error::OrderInvalid)?.split(',');
    let sequence = decimal(field(&mut fields, "event_sequence")?)?;
    let ordinal = decimal(field(&mut fields, "source_event_ordinal")?)?;
    let previous_event = digest(field(&mut fields, "previous_event_sha256")?)?;
    let previous_source_event = digest(field(&mut fields, "previous_source_event_sha256")?)?;
    let operation = digest(field(&mut fields, "operation_id")?)?;
    let state = match field(&mut fields, "state")? {
        "\"A\"" => LegacySourceMappingState::Active,
        "\"R\"" => LegacySourceMappingState::Retired,
        _ => return Err(Error::OrderInvalid),
    };
    let legacy_source = digest(field(&mut fields, "source_id")?)?;
    let legacy_revision = digest(field(&mut fields, "legacy_revision_id")?)?;
    let content = digest(field(&mut fields, "content_sha256")?)?;
    let source_bytes = decimal(field(&mut fields, "byte_length")?)?;
    let file_identity = digest(field(&mut fields, "file_identity_sha256")?)?;
    let path = digest(field(&mut fields, "path_sha256")?)?;
    let native_identity = match field(&mut fields, "identity_strength")? {
        "\"native\"" => true, "\"path-bound\"" => false, _ => return Err(Error::OrderInvalid),
    };
    let event = digest(field(&mut fields, "event_sha256")?)?;
    if fields.next().is_some() { return Err(Error::OrderInvalid); }
    Ok((LegacySourceMappingEvent { sequence, operation, legacy_source, legacy_revision,
        content, file_identity, path, event, previous_event,
        previous_source_event: (previous_source_event.as_bytes() != &[0; 32]).then_some(previous_source_event),
        source_bytes, state, native_identity }, ordinal))
}

fn field<'a>(fields: &mut core::str::Split<'a, char>, key: &str) -> Result<&'a str, Error> {
    let (name, value) = fields.next().and_then(|value| value.split_once(':')).ok_or(Error::OrderInvalid)?;
    if name.strip_prefix('"').and_then(|value| value.strip_suffix('"')) != Some(key) {
        return Err(Error::OrderInvalid);
    }
    Ok(value)
}

fn digest(value: &str) -> Result<Sha256Digest32, Error> {
    let value = value.strip_prefix('"').and_then(|value| value.strip_suffix('"')).ok_or(Error::OrderInvalid)?;
    Sha256Digest32::parse_hex(value).map_err(|_| Error::OrderInvalid)
}

fn decimal(value: &str) -> Result<u64, Error> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::OrderInvalid);
    }
    value.parse().map_err(|_| Error::OrderInvalid)
}

#[cfg(test)]
#[path = "mapping_codec/tests.rs"]
mod tests;
