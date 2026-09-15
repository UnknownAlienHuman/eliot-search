use super::{
    contains_vendor_sdk_reference, public_vendor_surface_lines,
    rust_string_constant,
};

#[test]
fn catches_imported_alias_in_multiline_public_function() {
    let source = r#"
use qdrant_client::qdrant::PointId;
pub fn leaked(
    point: PointId,
) {}
"#;
    assert_eq!(public_vendor_surface_lines(source), vec![3]);
}

#[test]
fn catches_public_trait_and_enum_members() {
    let trait_source = r#"
use qdrant_client::Qdrant;
pub trait Port {
    fn client(&self) -> Qdrant;
}
"#;
    let enum_source = r#"
use qdrant_client::qdrant::PointId;
pub enum ResultId {
    Vendor(PointId),
}
"#;
    assert_eq!(public_vendor_surface_lines(trait_source), vec![3]);
    assert_eq!(public_vendor_surface_lines(enum_source), vec![3]);
}

#[test]
fn follows_private_type_aliases_into_public_signatures() {
    let source = r#"
use qdrant_client::Qdrant;
type Client = Qdrant;
pub fn client() -> Client;
"#;
    assert_eq!(public_vendor_surface_lines(source), vec![4]);
}

#[test]
fn ignores_private_surfaces_comments_and_literals() {
    let private_source = r#"
use qdrant_client::Qdrant;
fn client() -> Qdrant { todo!() }
pub(crate) fn crate_client() -> Qdrant { todo!() }
"#;
    let inert_source = r##"
// qdrant_client::Qdrant
const NOTE: &str = "qdrant_client::Qdrant";
const RAW: &str = r#"qdrant_client::Qdrant"#;
"##;
    assert!(public_vendor_surface_lines(private_source).is_empty());
    assert!(!contains_vendor_sdk_reference(inert_source));
}

#[test]
fn catches_sdk_paths_with_arbitrary_token_spacing() {
    for source in [
        "type Client = qdrant_client :: Qdrant;",
        "type Client = qdrant_client\n::\nQdrant;",
        "type Client = qdrant_client/* boundary */::Qdrant;",
        "type Client = ::r#qdrant_client :: Qdrant;",
    ] {
        assert!(contains_vendor_sdk_reference(source), "{source}");
    }
}

#[test]
fn catches_multiline_grouped_and_renamed_crate_imports() {
    for source in [
        "use\nqdrant_client\nas sdk;",
        "use /* comment */ ::qdrant_client;",
        "use { qdrant_client as sdk };",
        "pub use { r#qdrant_client };",
        "extern\ncrate\nqdrant_client as sdk;",
        "extern crate r#qdrant_client as sdk;",
    ] {
        assert!(contains_vendor_sdk_reference(source), "{source}");
    }
}

#[test]
fn sdk_name_must_be_a_complete_reference_token() {
    for source in [
        "use qdrant_client_helpers::Client;",
        "use my_qdrant_client::Client;",
        "type Client = my_qdrant_client::Qdrant;",
        "fn flag(qdrant_client: bool) -> bool { qdrant_client }",
        "fn flag() { let r#use = 1; let qdrant_client = true; }",
        "use local::Client; fn flag() { let qdrant_client = true; }",
    ] {
        assert!(!contains_vendor_sdk_reference(source), "{source}");
    }
}

#[test]
fn version_extraction_ignores_commented_and_quoted_declarations() {
    let source = r##"
/*
pub const QUALIFIED_CLIENT_VERSION: &str = "9.9.9";
*/
const NOTE: &str = r#"
pub const QUALIFIED_CLIENT_VERSION: &str = "8.8.8";
"#;
pub const QUALIFIED_CLIENT_VERSION: &str = "1.2.3";
"##;
    assert_eq!(
        rust_string_constant(source, "QUALIFIED_CLIENT_VERSION"),
        Some("1.2.3".to_owned())
    );
}

#[test]
fn version_extraction_rejects_missing_or_duplicate_active_declarations() {
    for source in [
        "/*\npub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n*/",
        concat!(
            "pub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n",
            "pub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n",
        ),
        concat!(
            "pub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n",
            "pub const QUALIFIED_CLIENT_VERSION: &str = \"9.9.9\";\n",
        ),
    ] {
        assert_eq!(
            rust_string_constant(source, "QUALIFIED_CLIENT_VERSION"),
            None,
            "{source}"
        );
    }
}

#[test]
fn catches_vendor_types_in_exported_macros() {
    let direct = r#"
#[macro_export]
macro_rules! leaked {
    () => { qdrant_client::qdrant::PointId };
}
"#;
    let imported = r#"
use qdrant_client::qdrant::PointId;
#[macro_export(local_inner_macros)] macro_rules! leaked {
    [] => [PointId];
}
"#;
    let declarative = r#"
use qdrant_client::qdrant::PointId;
pub macro leaked() {
    PointId
}
"#;
    assert_eq!(public_vendor_surface_lines(direct), vec![2]);
    assert_eq!(public_vendor_surface_lines(imported), vec![3]);
    assert_eq!(public_vendor_surface_lines(declarative), vec![3]);
}

#[test]
fn private_macros_do_not_become_public_surfaces() {
    let source = r#"
use qdrant_client::qdrant::PointId;
macro_rules! private_adapter {
    () => { PointId };
}
"#;
    assert!(public_vendor_surface_lines(source).is_empty());
}

#[test]
fn catches_visibility_and_qualifiers_split_across_lines() {
    let source = r#"
use qdrant_client::qdrant::PointId;
pub
async
unsafe
fn leaked() -> PointId { unreachable!() }
"#;
    assert_eq!(public_vendor_surface_lines(source), vec![3]);
}
