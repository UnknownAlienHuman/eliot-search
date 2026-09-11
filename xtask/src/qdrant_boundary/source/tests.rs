use super::{contains_vendor_sdk_reference, public_vendor_surface_lines};

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
