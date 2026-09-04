//! One-shot persistent DIRECT search with explicit aggregate coverage.

use std::env;
use std::path::Path;
use std::process::ExitCode;

use crate::development::DataRootGuard;
use crate::direct_store::{DirectStore, StoreSearchResult};

/// Intercepts persistent search commands before the legacy one-shot dispatcher.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let command = arguments.first().and_then(|value| value.to_str())?;
    let ascii_insensitive = match command {
        "--search-root" => false,
        "--search-root-ascii-insensitive" => true,
        _ => return None,
    };
    let result = match arguments.as_slice() {
        [_, root, query] => query
            .to_str()
            .ok_or_else(|| "DIRECT_QUERY_NOT_UTF8".to_owned())
            .and_then(|query| run(Path::new(root), query, ascii_insensitive)),
        _ => Err("USAGE_ERROR".to_owned()),
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

fn run(root: &Path, query: &str, ascii_insensitive: bool) -> Result<(), String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    let result = store.search(query, ascii_insensitive)?;
    emit(&store.namespace_id(), &result);
    Ok(())
}

fn emit(namespace_id: &str, result: &StoreSearchResult) {
    println!(
        concat!(
            "{{\"event\":\"corpus_search_started\",",
            "\"namespace_id\":\"{}\",\"registered_sources\":{},",
            "\"active_sources\":{},\"source_backed\":true,",
            "\"durable_revision\":true,\"encrypted_at_rest\":false}}"
        ),
        namespace_id,
        result.registered_sources,
        result.active_sources,
    );
    for gap in &result.gaps {
        println!(
            concat!(
                "{{\"event\":\"source_gap\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"reason\":\"{}\"}}"
            ),
            gap.source_id, gap.revision_id, gap.reason,
        );
    }
    for item in &result.matches {
        println!(
            concat!(
                "{{\"event\":\"match\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                "\"path_digest\":\"{}\",\"evidence_id\":\"{}\",",
                "\"byte_start\":{},\"byte_end\":{},",
                "\"line\":{},\"column_bytes\":{},",
                "\"source_backed\":true}}"
            ),
            item.source_id,
            item.revision_id,
            item.content_digest,
            item.path_digest,
            item.evidence_id,
            item.byte_start,
            item.byte_end,
            item.line,
            item.column_bytes,
        );
    }
    println!(
        concat!(
            "{{\"event\":\"corpus_search_complete\",",
            "\"matches\":{},\"gaps\":{},\"searched_sources\":{},",
            "\"searched_bytes\":{},\"active_sources\":{},",
            "\"complete\":{},\"match_limit_reached\":{},",
            "\"source_budget_exhausted\":{},",
            "\"byte_budget_exhausted\":{},\"source_backed\":true,",
            "\"encrypted_at_rest\":false}}"
        ),
        result.matches.len(),
        result.gaps.len(),
        result.searched_sources,
        result.searched_bytes,
        result.active_sources,
        result.complete,
        result.match_limit_reached,
        result.source_budget_exhausted,
        result.byte_budget_exhausted,
    );
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
