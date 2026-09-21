//! Synthetic lexical fixtures. These neither compile vendor code nor qualify Qdrant.

use super::cross_file::{BridgeSource, find_cross_file_vendor_surfaces};
use super::imports::vendor_identifiers;
use super::lexer::code_only;
use super::public_vendor_surface_lines;
use super::statements::{StatementKind, collect_statements};
use super::use_tree::{collect_use_statements, direct_tainted_bindings};

const VENDOR: &str = "qdrant_client";
const BRIDGE: &str = "crates/search-index-qdrant/search-qdrant-bridge";

fn source(path: &str, text: &str) -> BridgeSource {
    BridgeSource::new(format!("{BRIDGE}/src/{path}"), text.to_owned())
}

#[test]
fn local_renamed_imports_do_not_depend_on_layout() {
    for import in [
        "use\tqdrant_client::Qdrant as Client;",
        "use\nqdrant_client::Qdrant\nas\nClient;",
        "use/* split */qdrant_client::Qdrant as Client;",
        "#[allow(unused)] use qdrant_client::Qdrant as Client;",
        "const N: u8 = 0; use qdrant_client::Qdrant as Client;",
        "use{qdrant_client::Qdrant as Client};",
    ] {
        let text = format!("{import}\npub fn leaked() -> Client;");
        assert_eq!(public_vendor_surface_lines(&text), vec![import.lines().count() + 1], "{text}");
    }
}

#[test]
fn renamed_extern_crates_are_token_delimited() {
    for import in [
        "extern\tcrate\tqdrant_client as sdk;",
        "extern\ncrate\nqdrant_client\nas\nsdk;",
        "#[allow(unused)] extern crate r#qdrant_client as sdk;",
    ] {
        let text = format!("{import}\nuse sdk::Qdrant as Client;\npub fn leaked() -> Client;");
        assert_eq!(public_vendor_surface_lines(&text), vec![import.lines().count() + 2]);
    }
}

#[test]
fn adjacent_declarations_do_not_contaminate_an_import() {
    let text = concat!(
        "use qdrant_client::Qdrant as Client; type Alias = Client; ",
        "type Owned = u8;\npub fn leaked() -> Alias;\npub fn safe() -> Owned;",
    );
    assert_eq!(public_vendor_surface_lines(text), vec![2]);
}

#[test]
fn mixed_groups_taint_only_vendor_leaves() {
    let text = concat!(
        "use { qdrant_client::Qdrant as Client, crate::owned::{Count, Flag} };\n",
        "pub fn safe() -> (Count, Flag);\npub fn leaked() -> Client;",
    );
    assert_eq!(public_vendor_surface_lines(text), vec![3]);
}

#[test]
fn similarly_named_modules_do_not_seed_vendor_aliases() {
    let text = "use qdrant_client_helpers::Qdrant as Client;\npub fn safe() -> Client;";
    assert!(public_vendor_surface_lines(text).is_empty());
}

#[test]
fn unicode_aliases_are_whole_tokens_not_ascii_substrings() {
    let text = concat!(
        "use qdrant_client::Qdrant as Клиент;\n",
        "pub fn safe() -> КлиентДанных;\npub fn leaked() -> Клиент;",
    );
    assert_eq!(public_vendor_surface_lines(text), vec![3]);
}

#[test]
fn raw_keyword_aliases_do_not_taint_unrelated_keywords() {
    let text = concat!(
        "use qdrant_client::Qdrant as r#type;\n",
        "pub type Owned = u8;\npub fn leaked() -> r#type;",
    );
    assert_eq!(public_vendor_surface_lines(text), vec![3]);
    let renamed = "use qdrant_client::{Qdrant as r#as};\npub fn leaked() -> r#as;";
    assert_eq!(public_vendor_surface_lines(renamed), vec![2]);
}

#[test]
fn type_aliases_include_generic_defaults_and_array_semicolons() {
    for alias in [
        "type\tAlias = Client;",
        "type\nAlias\n= Client;",
        "#[allow(unused)] type Alias = [Client; 2];",
        "type Alias<T = Client> = Vec<T>;",
        "type Alias<T: Bound<Client>> = Vec<T>;",
    ] {
        let text = format!("use qdrant_client::Qdrant as Client;\n{alias}\npub fn leaked() -> Alias;");
        assert_eq!(public_vendor_surface_lines(&text), vec![alias.lines().count() + 2]);
    }
}

#[test]
fn import_and_type_aliases_share_one_fixed_point() {
    let text = concat!(
        "use self::Alias as Client;\ntype Alias = sdk::Qdrant;\n",
        "use qdrant_client as sdk;\npub fn leaked() -> Client;",
    );
    assert_eq!(public_vendor_surface_lines(text), vec![4]);
}

#[test]
fn underscore_imports_do_not_create_a_referenceable_name() {
    let code = code_only("use qdrant_client::Qdrant as _;\npub fn safe() -> _;");
    assert!(!vendor_identifiers(&code, VENDOR).contains("_"));
    assert!(public_vendor_surface_lines(&code).is_empty());
}

#[test]
fn raw_keywords_literals_and_private_macro_bodies_are_not_declarations() {
    let text = r##"
const NOTE: &str = "use qdrant_client::Qdrant as Client;";
fn private() { let r#use = 0; let r#type = 1; }
macro_rules! private { () => { use qdrant_client::Qdrant as Client; }; }
pub fn safe() -> Client;
"##;
    assert!(public_vendor_surface_lines(text).is_empty());
}

#[test]
fn shared_collector_preserves_declaration_lines_and_visibility() {
    let code = "#[allow(unused)] pub\nuse crate::a::A; type B = [u8; 4];\npub (crate)\nuse crate::b::B;";
    let statements = collect_statements(code).expect("declarations");
    assert_eq!(statements.len(), 3);
    assert_eq!((statements[0].line, statements[0].public), (1, true));
    assert_eq!(statements[0].body.trim(), "crate::a::A");
    assert_eq!(statements[1].kind, StatementKind::TypeAlias);
    assert_eq!(statements[1].body.trim(), "B = [u8; 4]");
    assert_eq!((statements[2].line, statements[2].public), (3, false));
    assert!(statements.iter().all(|statement| statement.complete));
}

#[test]
fn extern_alias_matching_checks_the_actual_crate_not_the_alias_name() {
    let code = "extern crate other as qdrant_client;";
    assert!(direct_tainted_bindings(code, VENDOR).bindings.is_empty());
}

#[test]
fn incomplete_and_malformed_imports_fail_closed() {
    for text in [
        "use\tqdrant_client::Qdrant as Client",
        "use crate::owned::{A, B",
        "use crate::owned::A as;",
        "use crate::owned::A B;",
        "use crate::owned::A: B;",
        "use crate::owned::{A,,B};",
    ] {
        let sources = [source("lib.rs", text)];
        assert_eq!(
            find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
            vec![(format!("{BRIDGE}/src/lib.rs"), 1)],
            "{text}",
        );
    }
}

#[test]
fn cross_file_reexports_resolve_layout_independent_aliases() {
    let sources = [
        source("private.rs", "use\tqdrant_client as sdk;\npub(crate) type\nClient = sdk::Qdrant;"),
        source("facade.rs", "#[allow(unused)] pub\nuse crate::private::Client as PublicClient;"),
        source("lib.rs", "pub use crate::facade::PublicClient;"),
    ];
    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/facade.rs"), 1), (format!("{BRIDGE}/src/lib.rs"), 1)],
    );
}

#[test]
fn cross_file_private_import_and_split_alias_reach_a_public_signature() {
    let sources = [
        source("private.rs", "pub(crate) type Client = qdrant_client::Qdrant;"),
        source("api.rs", "use\tcrate::private::Client as C;\ntype\nAlias = C;\npub fn leaked() -> Alias;"),
    ];
    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/api.rs"), 4)],
    );
}

#[test]
fn public_globs_propagate_split_public_visibility_only() {
    for (visibility, expected) in [("pub\n", true), ("pub (crate)\n", false)] {
        let sources = [
            source("private.rs", &format!("{visibility}type Client = qdrant_client::Qdrant;")),
            source("lib.rs", "pub\tuse crate::private::*;"),
        ];
        let findings = find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR);
        assert_eq!(!findings.is_empty(), expected, "{visibility}: {findings:?}");
    }
}

#[test]
fn vendor_globs_still_fail_with_token_separated_imports() {
    let sources = [source("lib.rs", "#[allow(unused)] use\tqdrant_client::{qdrant::*};")];
    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 1)],
    );
}

#[test]
fn nested_groups_self_aliases_and_raw_bindings_are_parsed_exactly() {
    let statements = collect_use_statements("use qdrant_client::{self as sdk, qdrant::{PointId as r#type}};");
    let leaves = statements[0].leaves.as_ref().expect("bounded tree");
    assert_eq!(leaves.len(), 2);
    assert_eq!(leaves[0].path, vec!["qdrant_client"]);
    assert_eq!(leaves[0].binding, "sdk");
    assert_eq!(leaves[1].path, vec!["qdrant_client", "qdrant", "PointId"]);
    assert_eq!(leaves[1].binding, "type");
}

#[test]
fn capture_bounds_do_not_turn_into_malformed_use_imports() {
    let statements = collect_use_statements("pub fn value<T>() -> impl Trait + use<T> {}\nuse crate::owned::Value;");
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].line, 2);
    assert!(statements[0].leaves.is_some());
}

#[test]
fn untainted_alias_cycles_terminate_without_inventing_vendor_names() {
    let code = "type A = B; type B = A; use self::A as C;";
    let identifiers = vendor_identifiers(code, VENDOR);
    assert_eq!(identifiers.into_iter().collect::<Vec<_>>(), vec![VENDOR]);
}

#[test]
fn declaration_and_path_budgets_fail_instead_of_dropping_suffixes() {
    let mut code = "type A = u8;\n".repeat(65_536);
    assert_eq!(collect_statements(&code).expect("exact limit").len(), 65_536);
    code.push_str("use qdrant_client::Qdrant;");
    assert_eq!(collect_statements(&code).expect_err("over limit"), 65_537);
    assert!(collect_use_statements(&code)[0].leaves.is_none());

    let code = format!("use {}Client;", "nested::".repeat(257));
    assert!(collect_use_statements(&code)[0].leaves.is_none());
}

#[test]
fn long_reverse_alias_chain_and_a_reachable_cycle_terminate() {
    let mut code = String::new();
    for index in (1..=2_048).rev() {
        code.push_str(&format!("type A{index} = A{};\n", index - 1));
    }
    code.push_str("type A0 = qdrant_client::Qdrant;\nuse self::A2048 as Last;\n");
    let identifiers = vendor_identifiers(&code, VENDOR);
    assert!(identifiers.contains("A2048"));
    assert!(identifiers.contains("Last"));
    assert_eq!(identifiers.len(), 2_051);

    let code = "type A = (qdrant_client::Qdrant, B); type B = A; use self::B as C;";
    let identifiers = vendor_identifiers(code, VENDOR);
    assert!(identifiers.contains("A") && identifiers.contains("B") && identifiers.contains("C"));
}
