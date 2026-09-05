//! Source guard only: native execution is covered by the Windows library tests.

#[test]
fn known_identity_call_sites_do_not_restore_nightly_metadata_methods() {
    let sources = [
        ("development", include_str!("../src/development.rs")),
        ("direct_store", include_str!("../src/direct_store.rs")),
        ("sealed_file_reader", include_str!("../src/sealed_file_reader.rs")),
        ("sealed_root_identity", include_str!("../src/sealed_root_identity.rs")),
        ("sealed_owner_epoch", include_str!("../src/sealed_owner_epoch.rs")),
        ("sealed_store", include_str!("../src/sealed_store.rs")),
        ("sealed_transaction", include_str!("../src/sealed_transaction.rs")),
        ("service_state", include_str!("../src/service_state.rs")),
        ("sealed_direct", include_str!("../src/bin/eliot-search-sealed-direct.rs")),
    ];
    for (name, source) in sources {
        for forbidden in [".volume_serial_number(", ".file_index("] {
            assert!(!source.contains(forbidden), "{name} restores {forbidden}");
        }
        assert!(source.contains("eliot_searchd::native_file::observe"), "{name}");
    }
}
