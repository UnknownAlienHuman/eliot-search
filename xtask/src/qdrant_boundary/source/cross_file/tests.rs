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
fn follows_cross_file_import_through_a_local_alias() {
    let sources = [
        source(
            "private.rs",
            "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source(
            "api.rs",
            concat!(
                "use crate::private::VendorClient;\n",
                "type LocalClient = VendorClient;\n",
                "pub fn client() -> LocalClient;\n",
            ),
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/api.rs"), 3)]
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
fn follows_renamed_vendor_module_imports_across_files() {
    for import in [
        "use qdrant_client as sdk;\n",
        "use qdrant_client::{self as sdk};\n",
    ] {
        let sources = [
            source(
                "private.rs",
                &format!(
                    "{import}pub(crate) type Client = sdk::Qdrant;\n"
                ),
            ),
            source("lib.rs", "pub use crate::private::Client;\n"),
        ];
        assert_eq!(
            find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
            vec![(format!("{BRIDGE}/src/lib.rs"), 1)],
            "{import}"
        );
    }
}

#[test]
fn direct_vendor_glob_import_fails_closed() {
    let sources = [source("lib.rs", "use qdrant_client::*;\n")];
    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 1)]
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
fn follows_literal_include_into_the_including_module() {
    let sources = [
        source(
            "real.rs",
            concat!(
                "include!(\"real/hidden.rs\");\n",
                "pub fn client() -> IncludedClient;\n",
            ),
        ),
        source(
            "real/hidden.rs",
            "type IncludedClient = qdrant_client::Qdrant;\n",
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/real.rs"), 2)]
    );
}

#[test]
fn follows_path_attribute_module_identity() {
    let sources = [
        source(
            "lib.rs",
            concat!(
                "#[path = \"hidden.rs\"]\n",
                "mod private;\n",
                "pub use crate::private::VendorClient;\n",
            ),
        ),
        source(
            "hidden.rs",
            "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 3)]
    );
}

#[test]
fn catches_public_glob_reexports() {
    let sources = [
        source(
            "private.rs",
            "pub type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source(
            "lib.rs",
            "mod private;\npub use crate::private::*;\n",
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 2)]
    );
}

#[test]
fn catches_private_glob_import_used_by_a_public_signature() {
    let sources = [
        source(
            "private.rs",
            "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
        ),
        source(
            "api.rs",
            concat!(
                "use crate::private::*;\n",
                "pub fn client() -> VendorClient;\n",
            ),
        ),
    ];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/api.rs"), 2)]
    );
}

#[test]
fn public_glob_does_not_export_private_vendor_bindings() {
    let sources = [
        source(
            "private.rs",
            concat!(
                "use qdrant_client::Qdrant;\n",
                "type InternalClient = Qdrant;\n",
            ),
        ),
        source(
            "lib.rs",
            "mod private;\npub use crate::private::*;\n",
        ),
    ];

    assert!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR).is_empty()
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

#[test]
fn oversized_use_tree_is_rejected_fail_closed() {
    let mut statement = String::from("pub use crate::");
    statement.push_str(&"m::{".repeat(70));
    statement.push_str("VendorClient");
    statement.push_str(&"}".repeat(70));
    statement.push_str(";\n");
    let sources = [source("lib.rs", &statement)];

    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 1)]
    );
}

#[test]
fn unresolved_or_cyclic_include_graph_is_rejected_fail_closed() {
    let unresolved = [source("lib.rs", "include!(\"missing.rs\");\n")];
    assert_eq!(
        find_cross_file_vendor_surfaces(&unresolved, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 1)]
    );

    let cycle = [
        source("a.rs", "include!(\"b.rs\");\n"),
        source("b.rs", "include!(\"a.rs\");\n"),
    ];
    assert_eq!(
        find_cross_file_vendor_surfaces(&cycle, BRIDGE, VENDOR),
        vec![
            (format!("{BRIDGE}/src/a.rs"), 1),
            (format!("{BRIDGE}/src/b.rs"), 1),
        ]
    );
}

#[test]
fn recursive_module_graph_is_rejected_fail_closed() {
    let sources = [source(
        "lib.rs",
        "#[path = \"lib.rs\"]\nmod recursive;\n",
    )];
    assert_eq!(
        find_cross_file_vendor_surfaces(&sources, BRIDGE, VENDOR),
        vec![(format!("{BRIDGE}/src/lib.rs"), 1)]
    );
}
