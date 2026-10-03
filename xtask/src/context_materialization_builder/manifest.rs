//! Prospective `context_manifest_v1` projection and canonical TOML rendering.

use serde_json::{Map as JsonMap, Value, json};
use toml::Value as TomlValue;

use crate::accepted_evidence::accepted_evidence_digest_toml;
use crate::context_artifact::BundleBlock;
use crate::context_materialization::{
    ArtifactRef, INSTANCE_STATUS, MaterializationPlanError, OptionalSignature,
    SignatureValue, operation_id,
};
use crate::ticket_planner::exact_sha256_hex;

use super::model::{ArtifactReadback, CandidateInput, Selection};

/// Canonical prospective manifest products.
#[derive(Clone, Debug)]
pub(super) struct ManifestProjection {
    pub(super) accepted_handoffs: Vec<Value>,
    pub(super) operation_input: Value,
    pub(super) operation_id: String,
    pub(super) payload: Vec<u8>,
    pub(super) signed_payload_sha256: String,
    pub(super) manifest: Option<Vec<u8>>,
    pub(super) exact_record_file_sha256: Option<String>,
    pub(super) target_control_record_path: Option<String>,
}

pub(super) fn project(
    input: &CandidateInput,
    selection: &Selection,
) -> Result<ManifestProjection, MaterializationPlanError> {
    let accepted_handoffs = accepted_handoffs(input)?;
    let operation_input = operation_input(input, selection)?;
    let operation = operation_id(&operation_input);
    let payload = render_payload(
        &input.candidate,
        selection,
        &accepted_handoffs,
        &operation,
    )?;
    let signed_payload_sha256 = exact_sha256_hex(&payload);
    let both_present = selection.materializer_signature.state == "PRESENT"
        && selection.reviewer_signature.state == "PRESENT";
    if both_present {
        validate_signature_digest(
            &selection.materializer_signature,
            &signed_payload_sha256,
            "materializer_signature_ref",
        )?;
        validate_signature_digest(
            &selection.reviewer_signature,
            &signed_payload_sha256,
            "reviewer_signature_ref",
        )?;
    }
    let manifest = if both_present {
        Some(render_manifest(
            &payload,
            selection,
            &signed_payload_sha256,
        )?)
    } else {
        None
    };
    let exact_record_file_sha256 = manifest.as_deref().map(exact_sha256_hex);
    let package = candidate_text(&input.candidate, "/package/name")?;
    let target_control_record_path = exact_record_file_sha256
        .as_ref()
        .map(|digest| format!("swarm/context-manifests/{package}/{digest}.toml"));
    Ok(ManifestProjection {
        accepted_handoffs,
        operation_input,
        operation_id: operation,
        payload,
        signed_payload_sha256,
        manifest,
        exact_record_file_sha256,
        target_control_record_path,
    })
}

fn operation_input(
    input: &CandidateInput,
    selection: &Selection,
) -> Result<Value, MaterializationPlanError> {
    Ok(json!({
        "schema_version": 1,
        "operation_kind": "materialize_context_v1",
        "repository": "UnknownAlienHuman/eliot-search",
        "base_commit": candidate_text(&input.candidate, "/repository/base_commit")?,
        "candidate_id": candidate_text(&input.candidate, "/candidate_id")?,
        "candidate_sha256": candidate_text(&input.candidate, "/candidate_sha256")?,
        "bundle_sha256": candidate_text(&input.candidate, "/artifact_candidate/sha256")?,
        "context_id": selection.context_id.as_str(),
        "created_at": selection.created_at.as_str(),
        "materializer_identity": selection.materializer_identity.as_str(),
        "reviewer_identity": selection.reviewer_identity.as_str(),
        "artifact_ref": artifact_ref_json(&selection.artifact_ref),
        "artifact_readback": readback_json(&selection.readback),
    }))
}

fn accepted_handoffs(
    input: &CandidateInput,
) -> Result<Vec<Value>, MaterializationPlanError> {
    let base_commit = candidate_text(&input.candidate, "/repository/base_commit")?;
    let mut projections = Vec::new();
    for block in input
        .blocks
        .iter()
        .filter(|block| block.kind == "accepted_handoff")
    {
        projections.push(accepted_handoff_projection(block, base_commit)?);
    }
    projections.sort_by(|left, right| {
        left.get("package")
            .and_then(Value::as_str)
            .cmp(&right.get("package").and_then(Value::as_str))
    });
    Ok(projections)
}

fn accepted_handoff_projection(
    block: &BundleBlock,
    base_commit: &str,
) -> Result<Value, MaterializationPlanError> {
    let text = std::str::from_utf8(&block.content).map_err(|_| {
        handoff_invalid("accepted handoff is not strict UTF-8")
    })?;
    let record: TomlValue = toml::from_str(text).map_err(|error| {
        handoff_invalid(&format!("accepted handoff parse failed: {error}"))
    })?;
    if record.get("record_kind").and_then(TomlValue::as_str)
        != Some("package_handoff_v1")
        || record.get("status").and_then(TomlValue::as_str) != Some("ACCEPTED")
    {
        return Err(handoff_invalid("accepted handoff kind or status mismatch"));
    }
    let identity = table(&record, "identity")?;
    let accepted = table(&record, "accepted_code")?;
    let public = table(&record, "public_surface")?;
    let compatibility = table(&record, "compatibility")?;
    let metadata = block.metadata.as_object().ok_or_else(|| {
        handoff_mismatch("handoff block metadata is not an object")
    })?;
    let package = toml_text(identity, "package")?;
    let final_commit = toml_text(accepted, "final_commit")?;
    let api_schema_digest = toml_text(public, "api_schema_digest")?;
    for (field, expected) in [
        ("package", package),
        ("accepted_commit", final_commit),
        ("api_schema_digest", api_schema_digest),
    ] {
        if metadata.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(handoff_mismatch(&format!(
                "handoff block metadata differs: {field}"
            )));
        }
    }
    let configuration = public
        .get("configuration_digest")
        .and_then(TomlValue::as_table)
        .ok_or_else(|| handoff_invalid("configuration_digest is not OptionalV1"))?;
    let configuration_state = toml_text(configuration, "state")?;
    let configuration_value = toml_text(configuration, "value")?;
    if !matches!(configuration_state, "ABSENT" | "PRESEL")
        || (configuration_state == "ABSENT" && !configuration_value.is_empty())
        || (configuration_state == "PRESEL"
            && !crate::context_materialization::sha256_hex_valid(configuration_value))
    {
        return Err(handoff_invalid("configuration digest state/value is invalid"));
    }
    let evidence = record
        .get("evidence")
        .ok_or_else(|| handoff_invalid("accepted handoff evidence array is missing"))?;
    let evidence_digest = accepted_evidence_digest_toml(evidence).map_err(|error| {
        handoff_invalid(&format!("accepted evidence digest failed: {error}"))
    })?;
    let compatibility_class = toml_text(compatibility, "class")?;
    if !matches!(
        compatibility_class,
        "COMPATIBLE" | "ADDITIVE" | "BREAKING" | "INTERNAL_ONLY"
    ) {
        return Err(handoff_invalid("compatibility class is invalid"));
    }
    let path = json_text(metadata, "path")?;
    let git_blob_id = json_text(metadata, "git_blob_id")?;
    let exact_record_file_sha256 = json_text(metadata, "exact_record_file_sha256")?;
    Ok(json!({
        "package": package,
        "handoff_ref": {
            "repository": "UnknownAlienHuman/eliot-search",
            "commit": base_commit,
            "path": path,
            "git_blob_id": git_blob_id,
            "exact_record_file_sha256": exact_record_file_sha256,
            "record_kind": "package_handoff_v1",
        },
        "accepted_commit": final_commit,
        "api_schema_digest": api_schema_digest,
        "configuration_digest": {
            "state": configuration_state,
            "value": configuration_value,
        },
        "evidence_digest": evidence_digest,
        "compatibility": compatibility_class,
    }))
}

fn render_payload(
    candidate: &Value,
    selection: &Selection,
    handoffs: &[Value],
    operation: &str,
) -> Result<Vec<u8>, MaterializationPlanError> {
    let package = candidate_text(candidate, "/package/name")?;
    let stage = candidate_text(candidate, "/package/stage")?;
    let wave = candidate
        .pointer("/package/wave")
        .and_then(Value::as_i64)
        .ok_or_else(|| render_invalid("candidate package.wave is invalid"))?;
    let sources = candidate
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| render_invalid("candidate sources are invalid"))?;
    let fragments = candidate
        .get("registry_fragments")
        .and_then(Value::as_array)
        .ok_or_else(|| render_invalid("candidate registry_fragments are invalid"))?;
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "record_kind = \"context_manifest_v1\"".to_owned(),
        format!("status = {}", quote(INSTANCE_STATUS)),
        String::new(),
        "[identity]".to_owned(),
        format!("context_id = {}", quote(&selection.context_id)),
        format!("operation_id = {}", quote(operation)),
        format!("package = {}", quote(package)),
        format!("stage = {}", quote(stage)),
        format!("wave = {wave}"),
        format!(
            "base_commit = {}",
            quote(candidate_text(candidate, "/repository/base_commit")?)
        ),
        String::new(),
        "[draft]".to_owned(),
        format!("path = {}", quote(candidate_text(candidate, "/draft/path")?)),
        format!(
            "git_blob_id = {}",
            quote(candidate_text(candidate, "/draft/git_blob_id")?)
        ),
        format!(
            "exact_file_sha256 = {}",
            quote(candidate_text(candidate, "/draft/exact_file_sha256")?)
        ),
        String::new(),
        "[artifact]".to_owned(),
        format!("ref = {}", artifact_ref_inline(&selection.artifact_ref)),
        format!("sha256 = {}", quote(&selection.artifact_ref.sha256)),
        format!("bytes = {}", selection.artifact_ref.bytes),
        "format = \"ELIOT_SWARM_CONTEXT_1\"".to_owned(),
    ];
    for source in sources {
        lines.push(String::new());
        lines.push("[[sources]]".to_owned());
        push_source(&mut lines, source)?;
    }
    for fragment in fragments {
        lines.push(String::new());
        lines.push("[[registry_fragments]]".to_owned());
        push_fragment(&mut lines, fragment)?;
    }
    for handoff in handoffs {
        lines.push(String::new());
        lines.push("[[accepted_handoffs]]".to_owned());
        push_handoff(&mut lines, handoff)?;
    }
    lines.push(String::new());
    lines.push("[verification]".to_owned());
    lines.push(format!("source_count = {}", sources.len()));
    lines.push(format!("registry_fragment_count = {}", fragments.len()));
    lines.push(format!("accepted_handoff_count = {}", handoffs.len()));
    lines.push("readback_verified = true".to_owned());
    lines.push("forbidden_path_scan_passed = true".to_owned());
    lines.push(String::new());
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    Ok(rendered.into_bytes())
}

fn render_manifest(
    payload: &[u8],
    selection: &Selection,
    payload_digest: &str,
) -> Result<Vec<u8>, MaterializationPlanError> {
    let materializer = selection.materializer_signature.value.as_ref().ok_or_else(|| {
        render_invalid("materializer signature is absent in complete proposal")
    })?;
    let reviewer = selection.reviewer_signature.value.as_ref().ok_or_else(|| {
        render_invalid("reviewer signature is absent in complete proposal")
    })?;
    let mut output = payload.to_vec();
    let lines = [
        "[signature]".to_owned(),
        format!("created_at = {}", quote(&selection.created_at)),
        format!(
            "materializer_identity = {}",
            quote(&selection.materializer_identity)
        ),
        format!(
            "reviewer_identity = {}",
            quote(&selection.reviewer_identity)
        ),
        format!("record_sha256 = {}", quote(payload_digest)),
        format!(
            "materializer_signature_ref = {}",
            signature_inline(materializer)
        ),
        format!("reviewer_signature_ref = {}", signature_inline(reviewer)),
        String::new(),
    ];
    output.extend_from_slice(lines.join("\n").as_bytes());
    Ok(output)
}

fn push_source(lines: &mut Vec<String>, source: &Value) -> Result<(), MaterializationPlanError> {
    let object = object(source, "source")?;
    lines.push(format!("order = {}", integer_value(object, "order")?));
    lines.push(format!(
        "repository_path = {}",
        quote(string_value(object, "repository_path")?)
    ));
    lines.push(format!(
        "git_blob_id = {}",
        quote(string_value(object, "git_blob_id")?)
    ));
    lines.push(format!(
        "exact_sha256 = {}",
        quote(string_value(object, "exact_sha256")?)
    ));
    lines.push(format!(
        "exact_bytes = {}",
        integer_value(object, "exact_bytes")?
    ));
    lines.push(format!(
        "materialization = {}",
        quote(string_value(object, "materialization")?)
    ));
    lines.push(format!(
        "materialized_sha256 = {}",
        quote(string_value(object, "materialized_sha256")?)
    ));
    lines.push(format!(
        "materialized_bytes = {}",
        integer_value(object, "materialized_bytes")?
    ));
    Ok(())
}

fn push_fragment(
    lines: &mut Vec<String>,
    fragment: &Value,
) -> Result<(), MaterializationPlanError> {
    let object = object(fragment, "registry fragment")?;
    lines.push(format!("order = {}", integer_value(object, "order")?));
    for key in [
        "registry_path",
        "selector",
        "source_git_blob_id",
        "source_exact_sha256",
    ] {
        lines.push(format!("{key} = {}", quote(string_value(object, key)?)));
    }
    lines.push(format!(
        "selector_match_count = {}",
        integer_value(object, "selector_match_count")?
    ));
    lines.push(format!(
        "fragment_sha256 = {}",
        quote(string_value(object, "fragment_sha256")?)
    ));
    lines.push(format!(
        "fragment_bytes = {}",
        integer_value(object, "fragment_bytes")?
    ));
    Ok(())
}

fn push_handoff(
    lines: &mut Vec<String>,
    handoff: &Value,
) -> Result<(), MaterializationPlanError> {
    let object = object(handoff, "accepted handoff")?;
    lines.push(format!(
        "package = {}",
        quote(string_value(object, "package")?)
    ));
    let reference = object(
        object
            .get("handoff_ref")
            .ok_or_else(|| render_invalid("handoff_ref missing"))?,
        "handoff_ref",
    )?;
    lines.push(format!("handoff_ref = {}", record_ref_inline(reference)?));
    lines.push(format!(
        "accepted_commit = {}",
        quote(string_value(object, "accepted_commit")?)
    ));
    lines.push(format!(
        "api_schema_digest = {}",
        quote(string_value(object, "api_schema_digest")?)
    ));
    let configuration = object(
        object
            .get("configuration_digest")
            .ok_or_else(|| render_invalid("configuration_digest missing"))?,
        "configuration_digest",
    )?;
    lines.push(format!(
        "configuration_digest = {{ state = {}, value = {} }}",
        quote(string_value(configuration, "state")?),
        quote(string_value(configuration, "value")?)
    ));
    lines.push(format!(
        "evidence_digest = {}",
        quote(string_value(object, "evidence_digest")?)
    ));
    lines.push(format!(
        "compatibility = {}",
        quote(string_value(object, "compatibility")?)
    ));
    Ok(())
}

fn validate_signature_digest(
    signature: &OptionalSignature,
    digest: &str,
    label: &str,
) -> Result<(), MaterializationPlanError> {
    if let Some(value) = &signature.value {
        if value.signed_payload_sha256 != digest {
            return Err(MaterializationPlanError::new(
                "MATERIALIZATION_SIGNATURE_PAYLOAD_MISMATCH",
                format!("{label} signed payload digest differs"),
            ));
        }
    }
    Ok(())
}

fn artifact_ref_json(reference: &ArtifactRef) -> Value {
    json!({
        "store_profile_ref": reference.store_profile_ref.as_str(),
        "artifact_id": reference.artifact_id.as_str(),
        "bytes": reference.bytes,
        "sha256": reference.sha256.as_str(),
    })
}

fn readback_json(readback: &ArtifactReadback) -> Value {
    json!({
        "verified": true,
        "verifier_identity": readback.verifier_identity.as_str(),
        "verified_at": readback.verified_at.as_str(),
        "sha256": readback.sha256.as_str(),
        "bytes": readback.bytes,
    })
}

fn artifact_ref_inline(reference: &ArtifactRef) -> String {
    format!(
        "{{ store_profile_ref = {}, artifact_id = {}, bytes = {}, sha256 = {} }}",
        quote(&reference.store_profile_ref),
        quote(&reference.artifact_id),
        reference.bytes,
        quote(&reference.sha256),
    )
}

fn signature_inline(signature: &SignatureValue) -> String {
    format!(
        concat!(
            "{{ approval_profile_ref = {}, approval_artifact_ref = {}, ",
            "signed_payload_sha256 = {}, actor_identity = {} }}"
        ),
        quote(&signature.approval_profile_ref),
        artifact_ref_inline(&signature.approval_artifact_ref),
        quote(&signature.signed_payload_sha256),
        quote(&signature.actor_identity),
    )
}

fn record_ref_inline(
    reference: &JsonMap<String, Value>,
) -> Result<String, MaterializationPlanError> {
    let keys = [
        "repository",
        "commit",
        "path",
        "git_blob_id",
        "exact_record_file_sha256",
        "record_kind",
    ];
    let mut parts = Vec::with_capacity(keys.len());
    for key in keys {
        parts.push(format!("{key} = {}", quote(string_value(reference, key)?)));
    }
    Ok(format!("{{ {} }}", parts.join(", ")))
}

fn quote(value: &str) -> String {
    serde_json::to_string(value)
        .expect("serializing a TOML-compatible string cannot fail")
}

fn table<'a>(
    value: &'a TomlValue,
    key: &str,
) -> Result<&'a toml::map::Map<String, TomlValue>, MaterializationPlanError> {
    value
        .get(key)
        .and_then(TomlValue::as_table)
        .ok_or_else(|| handoff_invalid(&format!("handoff table is missing: {key}")))
}

fn toml_text<'a>(
    value: &'a toml::map::Map<String, TomlValue>,
    key: &str,
) -> Result<&'a str, MaterializationPlanError> {
    value
        .get(key)
        .and_then(TomlValue::as_str)
        .ok_or_else(|| handoff_invalid(&format!("handoff field is invalid: {key}")))
}

fn candidate_text<'a>(
    candidate: &'a Value,
    pointer: &str,
) -> Result<&'a str, MaterializationPlanError> {
    candidate
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| render_invalid(&format!("candidate field is invalid: {pointer}")))
}

fn object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a JsonMap<String, Value>, MaterializationPlanError> {
    value
        .as_object()
        .ok_or_else(|| render_invalid(&format!("{label} is not an object")))
}

fn string_value<'a>(
    object: &'a JsonMap<String, Value>,
    key: &str,
) -> Result<&'a str, MaterializationPlanError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| render_invalid(&format!("manifest field is not text: {key}")))
}

fn integer_value(
    object: &JsonMap<String, Value>,
    key: &str,
) -> Result<u64, MaterializationPlanError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| render_invalid(&format!("manifest field is not u64: {key}")))
}

fn json_text<'a>(
    object: &'a JsonMap<String, Value>,
    key: &str,
) -> Result<&'a str, MaterializationPlanError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| handoff_mismatch(&format!("handoff metadata field is invalid: {key}")))
}

fn handoff_invalid(message: &str) -> MaterializationPlanError {
    MaterializationPlanError::new("MATERIALIZATION_HANDOFF_INVALID", message)
}

fn handoff_mismatch(message: &str) -> MaterializationPlanError {
    MaterializationPlanError::new("MATERIALIZATION_HANDOFF_MISMATCH", message)
}

fn render_invalid(message: &str) -> MaterializationPlanError {
    MaterializationPlanError::new("MATERIALIZATION_RENDER_INVALID", message)
}
