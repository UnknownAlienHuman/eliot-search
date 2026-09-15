use super::{BridgeSource, find_cross_file_vendor_surfaces};

const BRIDGE: &str = "crates/search-index-qdrant/search-qdrant-bridge";
const VENDOR: &str = "qdrant_client";

fn source(path: &str, text: &str) -> BridgeSource {
    BridgeSource::new(format!("{BRIDGE}/src/{path}"), text.to_owned())
}

#[test]
fn catches_private_alias_reexported_from_the_crate_root() {
    let sources = [
        source(
            "private.rs",
            "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source(
            "lib.rs",
            "mod private;\npub use crate::private::VendorClient;\n",
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 2)]
    );
}

#[test]
fn catches_private_import_used_by_a_public_signature() {
    let sources = [
        source(
            "private.rs",
            "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source(
            "api.rs",
            concat!(
                "use crate::private::VendorClient as LocalClient;\n",
                "pub fn client() -> LocalClient;\n",
            ),
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/api.rs"), 2)]
    );
}

#[test]
fn follows_chained_and_grouped_reexports_to_the_root() {
    let sources = [
        source(
            "private.rs",
            concat!(
                "pub(crate) type Client = qdrant_client::Qdrant;\n",
                "pub(crate) type Point = qdrant_client::qdrant::PointId;\n",
            ),
        ),
        source(
            "facade.rs",
            "pub use crate::private::{Client as BridgeClient, Point};\n",
        ),
        source(
            "lib.rs",
            "pub use crate::facade::{BridgeClient, Point as PublicPoint};\n",
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![
            (format!("{BRIDGE}/src/facade.rs"), 1),
            (format!("{BRIDGE}/src/lib.rs"), 1),
        ]
    );
}

#[test]
fn follows_renamed_extern_crate_imports_across_files() {
    let sources = [
        source(
            "private.rs",
            concat!(
                "extern crate qdrant_client as sdk;\n",
                "pub(crate) use sdk::Qdrant as Client;\n",
            ),
        ),
        source(
            "lib.rs",
            "pub use crate::private::Client;\n",
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 1)]
    );
}

#[test]
fn unrelated_same_named_items_do_not_become_tainted() {
    let sources = [
        source(
            "private.rs",
            "type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source("owned.rs", "pub struct VendorClient;\n"),
        source(
            "lib.rs",
            "pub use crate::owned::VendorClient;\n",
        ),
    ];

    assert!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR).is_empty()
    );
}

#[test]
fn private_cross_file_import_without_public_use_is_not_a_surface() {
    let sources = [
        source(
            "private.rs",
            "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source(
            "internal.rs",
            concat!(
                "use crate::private::VendorClient;\n",
                "fn client() -> VendorClient { todo!() }\n",
            ),
        ),
    ];

    assert!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR).is_empty()
    );
}
