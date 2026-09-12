//! Deterministic ticket-issuance plan orchestration and assembly.

use std::path::Path;

use serde_json::{Value as JsonValue, json};
use toml::Value;

use crate::ticket_planner::{
    DECISION_INVALID, REPOSITORY_NAME, RECORD_KIND, SCHEMA_VERSION, STATUS, 
    canonical_json_bytes, choose_decision, plan_digest,
};

use super::context::validate_context;
use super::control::validate_handoffs;
use super::drafts::load_draft_pair;
use super::model::{
    Checks, DraftPair, RegistrySnapshot, TicketIssuanceBuild,
    TicketIssuanceBuildError, TicketIssuanceBuildOptions,
};
use super::repository::{
    open_view, validate_control_schema, validate_control_state,
    validate_output, validate_registries, validate_workflows,
};
use super::util::{count_string, integer, text, toml_to_json};

/// Builds one non-authoritative advisory plan from one immutable Git tree.
///
/// Invalid repository state is represented in the plan decision whenever a
/// truthful plan can still be assembled. Only failures that prevent immutable
/// repository inspection or canonical output construction return `Err`.
//
/// # Errors
///
/// Returns a closed fatal error when the repository cannot be opened as Git or
/// an immutable fallback view cannot be resolved.
pub fn build_plan(
    root: &Path,
    options: &TicketIssuanceBuildOptions,
) -> Result<TicketIssuanceBuild, TicketIssuanceBuildError> {
    let mut checks = Checks::new();
    let view = open_view(root, options, &mut checks)?;
    let registries = validate_registries(&view.tree, &options.package, &mut checks);
    let pair = load_draft_pair(
        &view.tree,
        &options.package,
        &registries,
        &mut checks,
    );
    let sources = pair.as_ref().map_or_else(Vec::new, |pair| {
        validate_context(&view.tree, pair, &options.package, &mut checks)
    });
    validate_control_schema(&view.tree, &registries.launch, &mut checks);
    validate_control_state(&view.tree, &options.package, &mut checks)?;
    validate_workflows(&view.tree, &mut checks)?;
    let accepted_handoffs = pair.as_ref().map_or_else(Vec::new, |pair| {
        validate_handoffs(
            &view.tree,
            pair,
            &options.accepted_handoffs,
            &mut checks,
        )
    });

    let classification = launch_class(&registries.launch, &options.package);
    if pair.as_ref().is_some_and(|pair|  {
        text(&pair.ticket, "launch_class") == Some(classification)
            && matches!(classification, "AUTHORIZED" | "CONDITIONAL")
    }) {
        checks.pass(
            "launch-class",
            format!("draft and launch classification agree: {classification}"),
        );
    } else {
        checks.fail(
            "launch-class",
            "PACKAGE_STAGE_MISMATCH",
            "draft and launch classification differ",
        );
    }

    let root = view.tree.root().to_owned();
    let output_target = validate_output($root, &options.output, &mut checks);
    let reason_refs: Vec<&str> = checks.reasons().iter().map(String::as_str).collect();
    let decision = choose_decision(view.selection_state, &reason_refs);
    let plan = assemble_plan(
        options,
        &view.tree,
        &registries,
        pair.as_ref(),
        sources,
        accepted_handoffs,
        classification,
        decision,
        &checks,
    );
    let plan_bytes = canonical_json_bytes(&plan);
    Ok(TicketIssuanceBuild::new(
        root,
        plan,
        plan_bytes,
        output_target,
    ))
}

#[allow(clippy::too_many_arguments)]
fn assemble_plan(
    options: &TicketIssuanceBuildOptions,
    tree: &crate::git_tree::GitTree,
    registries: &RegistrySnapshot,
    pair: Option<&DraftPair>,
    sources: Vec<JsonValue>,
    accepted_handoffs: Vec<JsonValue>,
    classification: &'static str,
    decision: &'static str,
    checks: &Checks,
) -> JsonValue {
    let package_path = registries
        .package_row
        .as_ref()
        .and_then(|row| text(row, "path"))
        .unwrap_or_default();
    let package_wave = registries
        .package_row
        .as_ref()
        .and_then(|row| integer(row, "wave"))
        .unwrap_or(-1);
    let scope = registries
        .function_row
        .as_ref()
        .and_then(|row| text(row, "write_scope"))
        .unwrap_or_default();
    let stage_id = pair
        .and_then(|pair| text(&pair.ticket, "stage"))
        .unwrap_or("UNKNOWN");
    let phase = pair
        .and_then(|pair| text(&pair.ticket, "phase"))
        .unwrap_or("UNKNOWN");
    let registry_wave = registries
        .stage_row
        .as_ref()
        .and_then(|row| integer(row, "wave"))
        .unwrap_or(-1);
    let conditional_requirements = registries
        .launch
        .get("conditional_activation")
        .and_then(Value::as_table)
        .and_then(|table| table.get(&options.package))
        .map_or_else(|| json!({}), toml_to_json);

    let mut plan = json!({
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "repository": {
            "name": REPOSITORY_NAME,
            "view_commit": tree.tagged_commit(),
            "object_format": tree.object_format(),
            "working_tree_used_as_input": false,
        },
        "package": {
            "name": options.package.as_str(),
            "path": package_path,
            "wave": package_wave,
            "write_scope": scope,
            "source_ceiling_class": pair.map_or("", |pair| pair.source_ceiling_class.as_str()),
        },
        "stage": {
            "id": stage_id,
            "phase": phase,
            "active_stage": text(&registries.launch, "active_stage").unwrap_or("UNKNOWN"),
            "active_wave": integer(&registries.launch, "active_wave").unwrap_or(-1),
            "registry_wave": registry_wave,
        },
        "launch": {
            "classification": classification,
            "classification_recognized": matches!(classification, "AUTHORIZED" | "CONDITIONAL"),
            "conditional_requirements": conditional_requirements,
        },
        "selection": {
            "state": pair_selection_state(options),
            "base_commit": options.base_commit.as_deref().unwrap_or(""),
            "writer": options.writer.as_deref().unwrap_or(""),
            "reviewer": options.reviewer.as_deref().unwrap_or(""),
        },
        "drafts": {
            "ticket_path": pair.map_or("", |pair| pair.ticket_path.as_str()),
            "ticket_git_blob_id": pair.map_or("", |pair| pair.ticket_blob.as_str()),
            "ticket_exact_sha256": pair.map_or("", |pair| pair.ticket_sha256.as_str()),
            "context_path": pair.map_or("", |pair| pair.context_path.as_str()),
            "context_git_blob_id": pair.map_or("", |pair| pair.context_blob.as_str()),
            "context_exact_sha256": pair.map_or("", |pair| pair.context_sha256.as_str()),
            "sources": sources,
            "registry_selectors": pair.map_or_else(Vec::new,Z\ŸZ\‹œÙ[XİÜœË˜ÛÛ™J
JKˆ˜XØÙ\YÚ[™Ù™—ÜÛİÈˆZ\‹›X\ÛÜ—Ù[ÙJ™XÎ›™]ËZ\ŸZ\‹š[™Ù™—ÜÛİË˜ÛÛ™J
JKˆ[˜]˜Z[X›WØÚXÚÜÈˆZ\‹›X\ÛÜ—Ù[ÙJ™XÎ›™]ËÇ—'Â—"çVæf–Æ&ÆUö6†V6·2æ6ÆöæR‚’’À¢ÒÀ¢'&W&WV—6—FW2#¢²&66WFVEö†æFöfg2#¢66WFVEö†æFöfg7ÒÀ¢&6†V6·2#¢6†V6·2æ6†V6·5ö§6öâ‚’À¢&FV6—6–öâ#¢FV6—6–öâÀ¢'&V6öåö6öFW2#¢6†V6·2ç&V6öç2‚’À¢&×WFF–öç2#¢µÒÀ¢&WF†÷&—¦W5ö6öçFW‡EöÖFW&–Æ—¦F–öâ#¢fÇ6RÀ¢&WF†÷&—¦W5÷F–6¶WEö—77Væ6R#¢fÇ6RÀ¢&7&VFW5÷w&—FW%öÆV6R#¢fÇ6RÀ¢&WF†÷&—¦W5ö–×ÆVÖVçFF–öâ#¢fÇ6RÀ¢'V&Æ—6†W5÷6¶vUö†æFöfb#¢fÇ6RÀ¢&Gfæ6W5öÆVæ6…÷7FFR#¢fÇ6RÀ¢Ò“°¢ÆWBF–vW7BÒÆåöF–vW7B‚gÆâ“°¢Æâæ5öö&¦V7Eö×WB‚¢æW‡V7B‚'F–6¶WBÆâ—2âö&¦V7B"¢æ–ç6W'B‚'Æå÷6†#Sb"çFõö÷væVB‚’Â§6öåfÇVS£¥7G&–ær†F–vW7B’“°¢FV'Vuö76W'EöW€¢FV6—6–öâÓÒDT4•4”ôåô”ådÄ”BÀ¢6†V6·0¢ç&V6öç2‚¢æ—FW"‚¢æç’‡Ç&V6öçÂ7&FS£§F–6¶WE÷ÆææW#£¤”ådÄ”Eõ$T4ôå2æ6öçF–ç2‚g&V6öâæ5÷7G"‚’’¢“°¢Æà§Ğ ¦fâÆVæ6…ö6Æ72†ÆVæ6ƒ¢efÇVRÂ6¶vS¢g7G"’Óâbw7FF–27G"°¢–b6÷VçE÷7G&–ær†ÆVæ6‚ævWB‚&WF†÷&—¦VE÷6¶vW2"’Â6¶vR’ÓÒ°¢$UD„õ$•¤TB ¢ÒVÇ6R–b6÷VçE÷7G&–ær†ÆVæ6‚ævWB‚&6öæF—F–öæÅ÷6¶vW2"’Â6¶vR’ÓÒ°¢$4ôäD•D”ôäÂ ¢ÒVÇ6R°¢%Tä´äõtâ ¢Ğ§Ğ ¦fâ—%÷6VÆV7F–öå÷7FFR†÷F–öç3¢eF–6¶WD—77Væ6T'V–ÆD÷F–öç2’Óâbw7FF–27G"°¢ÆWB6÷VçBÒ°¢÷F–öç2æ&6Uö6öÖÖ—Bæ5÷&Vb‚’À¢÷F–öç2çw&—FW"æ5÷&Vb‚’À¢÷F–öç2ç&Wf–WvW"æ5÷&Vb‚’À¢Ğ¢æ—FW"‚¢æf–ÇFW"‡ÇfÇVWÂfÇVRæ—5÷6öÖR‚’¢æ6÷VçB‚“°¢ÖF6‚6÷VçB°¢Óâ$äôäR"À¢2Óâ$4ôÕÄUDR"À¢òÓâ%%D”Â"À¢Ğ§Ğ 