use serde_json::{Value, json};
use xtask::ticket_planner::{SelectorDocs, SelectorStatus, resolve_selector_with_w0_module};

fn resolve(
    module: Option<&Value>,
    selector: &str,
    package: &str,
) -> (SelectorStatus, &'static str) {
    let docs = SelectorDocs {
        crates: None,
        functions: None,
        stages: None,
        launch: None,
    };
    resolve_selector_with_w0_module(&docs, module, selector, package)
}

fn w0_module(packages: &[&str]) -> Value {
    json!({
        "schema_version": 1,
        "project": "eliot-search",
        "earliest_wave": 0,
        "package": packages.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
    })
}

fn committed_w0_module() -> Value {
    let module = toml::from_str::<toml::Value>(include_str!("../../swarm/modules/w0.toml"))
        .expect("committed W0 module packet is valid TOML");
    serde_json::to_value(module).expect("committed W0 packet converts to a JSON value")
}

const SEARCH_CONTRACTS: &str = "swarm/modules/w0.toml::package[name=search-contracts]";

#[test]
fn unique_matching_package_in_committed_w0_packet_resolves() {
    let module = committed_w0_module();

    assert_eq!(
        resolve(Some(&module), SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::Ok
    );
}

#[test]
fn missing_module_document_or_matching_row_is_not_unique() {
    assert_eq!(
        resolve(None, SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::NotUnique
    );

    let module = w0_module(&["search-domain"]);
    assert_eq!(
        resolve(Some(&module), SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::NotUnique
    );
}

#[test]
fn duplicate_matching_w0_package_is_not_unique() {
    let module = w0_module(&["search-contracts", "search-contracts"]);

    assert_eq!(
        resolve(Some(&module), SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::NotUnique
    );
}

#[test]
fn mismatched_package_identity_is_unsupported() {
    let module = w0_module(&["search-contracts", "search-domain"]);

    assert_eq!(
        resolve(
            Some(&module),
            "swarm/modules/w0.toml::package[name=search-domain]",
            "search-contracts"
        )
        .0,
        SelectorStatus::Unsupported
    );
}

#[test]
fn wrong_module_path_header_identity_or_expression_is_unsupported() {
    let module = w0_module(&["search-contracts"]);

    assert_eq!(
        resolve(
            Some(&module),
            "swarm/modules/w1.toml::package[name=search-contracts]",
            "search-contracts"
        )
        .0,
        SelectorStatus::Unsupported
    );
    assert_eq!(
        resolve(
            Some(&module),
            "swarm/modules/w0.toml::stage[id=W0]",
            "search-contracts"
        )
        .0,
        SelectorStatus::Unsupported
    );
    assert_eq!(
        resolve(
            Some(&module),
            "swarm/modules/w0.toml::package[name=search-contracts]::extra",
            "search-contracts"
        )
        .0,
        SelectorStatus::Unsupported
    );
    assert_eq!(
        resolve(
            Some(&module),
            "swarm/modules/w0.toml::package[name=search-contracts",
            "search-contracts"
        )
        .0,
        SelectorStatus::Unsupported
    );

    let wrong_wave = json!({
        "schema_version": 1,
        "project": "eliot-search",
        "earliest_wave": 1,
        "package": [{"name": "search-contracts"}],
    });
    assert_eq!(
        resolve(Some(&wrong_wave), SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::Unsupported
    );

    let wrong_project = json!({
        "schema_version": 1,
        "project": "other-project",
        "earliest_wave": 0,
        "package": [{"name": "search-contracts"}],
    });
    assert_eq!(
        resolve(Some(&wrong_project), SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::Unsupported
    );

    let wrong_schema = json!({
        "schema_version": 2,
        "project": "eliot-search",
        "earliest_wave": 0,
        "package": [{"name": "search-contracts"}],
    });
    assert_eq!(
        resolve(Some(&wrong_schema), SEARCH_CONTRACTS, "search-contracts").0,
        SelectorStatus::Unsupported
    );
}

#[test]
fn historical_selector_forms_still_use_the_existing_grammar() {
    let crates = json!({"package": [{"name": "search-contracts"}]});
    let docs = SelectorDocs {
        crates: Some(&crates),
        functions: None,
        stages: None,
        launch: None,
    };

    assert_eq!(
        resolve_selector_with_w0_module(
            &docs,
            None,
            "swarm/crates.toml::package[name=search-contracts]",
            "search-contracts"
        )
        .0,
        SelectorStatus::Ok
    );
}
