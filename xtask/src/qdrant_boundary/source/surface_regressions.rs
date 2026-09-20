//! Lexical fixtures, not compiled vendor programs or live-Qdrant qualification.

use super::lexer::code_only;
use super::public_vendor_surface_lines;
use super::surface::find_public_surfaces_matching;
use super::tokens::{code_token_spans, code_tokens};

#[test]
fn public_headers_are_independent_of_whitespace_and_attributes() {
    for source in [
        "pub\tfn leaked() -> qdrant_client::Qdrant {}",
        "pub\r\nfn leaked() -> qdrant_client::Qdrant {}",
        "pub/* visibility */fn leaked() -> qdrant_client::Qdrant {}",
        "#[inline] pub fn leaked() -> qdrant_client::Qdrant {}",
        "fn private() {} pub fn leaked() -> qdrant_client::Qdrant {}",
        "pub\tunsafe\textern \"C\" fn leaked() -> qdrant_client::Qdrant {}",
    ] {
        assert_eq!(public_vendor_surface_lines(source), vec![1], "{source}");
    }
}

#[test]
fn multiline_named_fields_include_the_entire_type() {
    let source = "pub struct Port {\n    pub client:\n        Option<\n            qdrant_client::Qdrant,\n        >,\n}";
    assert_eq!(public_vendor_surface_lines(source), vec![2]);
    let alias = "use qdrant_client::Qdrant;\npub struct Port {\n    pub\n    client:\n        Qdrant,\n}";
    assert_eq!(public_vendor_surface_lines(alias), vec![3]);
}

#[test]
fn compact_public_fields_are_separate_from_private_fields() {
    let safe = "pub struct Port { private: qdrant_client::Qdrant, pub count: usize }";
    let leak = "pub struct Port { private: usize, pub client: qdrant_client::Qdrant }";
    assert!(public_vendor_surface_lines(safe).is_empty());
    assert_eq!(public_vendor_surface_lines(leak), vec![1]);
}

#[test]
fn grouped_public_imports_are_not_cut_off_at_the_opening_brace() {
    let source = "pub\nuse {\n    qdrant_client::qdrant::PointId,\n};";
    assert_eq!(public_vendor_surface_lines(source), vec![1]);
    let same_line = "pub use { qdrant_client::qdrant::PointId };";
    assert_eq!(public_vendor_surface_lines(same_line), vec![1]);
}

#[test]
fn split_traits_and_enums_include_their_members() {
    for source in [
        "pub\ntrait Port {\nfn client(&self) -> qdrant_client::Qdrant;\n}",
        "pub\tunsafe\ttrait Port {\nfn client(&self) -> qdrant_client::Qdrant;\n}",
        "#[non_exhaustive] pub\tenum Value {\nVendor(qdrant_client::Qdrant),\n}",
    ] {
        assert_eq!(public_vendor_surface_lines(source), vec![1], "{source}");
    }
}

#[test]
fn public_function_bodies_do_not_taint_vendor_neutral_signatures() {
    for source in [
        "pub fn count() -> usize { qdrant_client::Qdrant::count() }",
        "pub fn count() -> usize {\n    qdrant_client::Qdrant::count()\n}",
        "use qdrant_client::Qdrant;\npub fn count() -> usize { Qdrant::count() }",
        "pub fn count() {} fn private() -> qdrant_client::Qdrant {}",
        "pub struct Count; fn private() -> qdrant_client::Qdrant {}",
    ] {
        assert!(public_vendor_surface_lines(source).is_empty(), "{source}");
    }
}

#[test]
fn constant_initializers_do_not_taint_the_declared_type() {
    for source in [
        "pub const COUNT: usize = qdrant_client::LIMIT;",
        "pub static COUNT: usize = { qdrant_client::LIMIT };",
    ] {
        assert!(public_vendor_surface_lines(source).is_empty(), "{source}");
    }
    let leak = "pub const CLIENT: Option<qdrant_client::Qdrant> = None;";
    assert_eq!(public_vendor_surface_lines(leak), vec![1]);
}

#[test]
fn constant_functions_are_still_checked_as_functions() {
    for source in [
        "pub const fn client() -> qdrant_client::Qdrant {}",
        "pub const unsafe fn client() -> qdrant_client::Qdrant {}",
    ] {
        assert_eq!(public_vendor_surface_lines(source), vec![1], "{source}");
    }
}

#[test]
fn nested_generic_commas_and_function_arrows_do_not_end_a_field() {
    for source in [
        "pub struct Port {\npub client: Pair<fn() -> u8,\nqdrant_client::Qdrant>,\n}",
        "pub struct Port {\npub callback: fn(u8,\nqdrant_client::Qdrant),\n}",
        "pub struct Port {\npub client: (u8,\nqdrant_client::Qdrant),\n}",
    ] {
        assert_eq!(public_vendor_surface_lines(source), vec![2], "{source}");
    }
}

#[test]
fn const_generic_groups_do_not_end_a_signature() {
    let source = "pub fn client() -> Array<{ 1 < 2 },\nqdrant_client::Qdrant> {}";
    assert_eq!(public_vendor_surface_lines(source), vec![1]);
    let field = "pub struct Port {\npub client: Array<{ 1 > 0 },\nqdrant_client::Qdrant>,\n}";
    assert_eq!(public_vendor_surface_lines(field), vec![2]);
}

#[test]
fn tuple_struct_where_clauses_remain_in_the_checked_signature() {
    let source = "pub struct Port<T>(pub T)\nwhere T: qdrant_client::VendorTrait;";
    assert_eq!(public_vendor_surface_lines(source), vec![1]);
    let named = "pub struct Port<T> where T: Fn() -> qdrant_client::Qdrant {}";
    assert_eq!(public_vendor_surface_lines(named), vec![1]);
}

#[test]
fn restricted_visibility_is_not_exported() {
    for visibility in ["pub(crate)", "pub ( super )", "pub\n(in crate::adapter)"] {
        let source = format!("{visibility} fn client() -> qdrant_client::Qdrant {{}}");
        assert!(public_vendor_surface_lines(&source).is_empty(), "{source}");
    }
}

#[test]
fn exported_macro_attributes_allow_spacing_and_intervening_attributes() {
    let source = "# [ macro_export ]\n#[allow(unused)]\nmacro_rules ! client {\n() => { qdrant_client::Qdrant };\n}";
    assert_eq!(public_vendor_surface_lines(source), vec![1]);
    let declarative = "pub\nmacro client(\n) { qdrant_client::Qdrant }";
    assert_eq!(public_vendor_surface_lines(declarative), vec![1]);
}

#[test]
fn exported_macros_nested_in_function_bodies_are_not_skipped() {
    let source = "pub fn install() {\n#[macro_export]\nmacro_rules! client { () => { qdrant_client::Qdrant }; }\n}";
    assert_eq!(public_vendor_surface_lines(source), vec![2]);
}

#[test]
fn macro_checks_end_at_the_actual_body_boundary() {
    let private = "macro_rules! private { () => { pub fn f() -> qdrant_client::Qdrant {} }; }";
    assert!(public_vendor_surface_lines(private).is_empty());
    let safe = "#[macro_export] macro_rules! safe { () => { 1 }; } fn private() -> qdrant_client::Qdrant {}";
    assert!(public_vendor_surface_lines(safe).is_empty());
}

#[test]
fn masked_text_and_raw_keyword_identifiers_cannot_create_public_items() {
    let source = r##"
// pub fn leaked() -> qdrant_client::Qdrant {}
const NOTE: &str = r#"pub fn leaked() -> qdrant_client::Qdrant {}"#;
fn private() { let r#pub = qdrant_client::LIMIT; }
"##;
    assert!(public_vendor_surface_lines(source).is_empty());
}

#[test]
fn unterminated_public_surfaces_are_checked_at_eof() {
    for source in [
        "pub\tfn client() -> qdrant_client::Qdrant",
        "pub struct Port {\npub client:\nqdrant_client::Qdrant",
    ] {
        assert!(!public_vendor_surface_lines(source).is_empty(), "{source}");
    }
}

#[test]
fn shared_surface_matcher_receives_only_the_qualified_public_type() {
    let source = "pub struct Port {\npub client:\ncrate::adapter::Client,\n}\npub fn safe() { crate::adapter::Client::new(); }";
    let code = code_only(source);
    assert_eq!(
        find_public_surfaces_matching(&code, |surface| surface.contains("crate::adapter::Client")),
        vec![2],
    );
}

#[test]
fn token_spans_preserve_unicode_offsets_raw_identifiers_and_lines() {
    let source = " \nαβ\t r#pub\r\n::Qdrant";
    let tokens = code_token_spans(source).collect::<Vec<_>>();
    assert_eq!(
        tokens.iter().map(|token| token.text).collect::<Vec<_>>(),
        vec!["αβ", "r#pub", ":", ":", "Qdrant"],
    );
    assert_eq!(tokens.iter().map(|token| token.line).collect::<Vec<_>>(), vec![2, 2, 3, 3, 3]);
    for token in &tokens {
        assert_eq!(&source[token.start..token.end], token.text);
    }
    assert_eq!(
        code_tokens(source).collect::<Vec<_>>(),
        tokens.iter().map(|token| token.text).collect::<Vec<_>>(),
    );
}
