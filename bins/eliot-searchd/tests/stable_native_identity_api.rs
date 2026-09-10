//! Source guard only: native execution is covered by the Windows library tests.
//!
//! T07 ownership: primary ingestion (`development`, `direct_store`) observes
//! no native identity directly; it reads through `safe_reader_adapter` into
//! the shared safe-reader kernel. Only the adapter and the sealed helpers
//! below may call `native_file::observe`/`hardlink_count`, and nobody may
//! restore nightly metadata methods.

#[test]
fn known_identity_call_sites_do_not_restore_nightly_metadata_methods() {
    let direct = [
        ("safe_reader_adapter", include_str!("../src/safe_reader_adapter.rs")),
        ("sealed_file_reader", include_str!("../src/sealed_file_reader.rs")),
        ("sealed_root_identity", include_str!("../src/sealed_root_identity.rs")),
        ("sealed_owner_epoch", include_str!("../src/sealed_owner_epoch.rs")),
        ("sealed_store", include_str!("../src/sealed_store.rs")),
        ("sealed_transaction", include_str!("../src/sealed_transaction.rs")),
        ("sealed_direct", include_str!("../src/bin/eliot-search-sealed-direct.rs")),
    ];
    for (name, source) in direct {
        for forbidden in [".volume_serial_number(", ".file_index("] {
            assert!(!source.contains(forbidden), "{name} restores {forbidden}");
        }
        assert!(source.contains("eliot_searchd::native_file::observe"), "{name}");
    }
    // Primary ingestion must go through the kernel adapter, never around it.
    let via_adapter = [
        ("development", include_str!("../src/development.rs")),
        ("direct_store", include_str!("../src/direct_store.rs")),
    ];
    for (name, source) in via_adapter {
        for forbidden in [".volume_serial_number(", ".file_index("] {
            assert!(!source.contains(forbidden), "{name} restores {forbidden}");
        }
        assert!(
            !source.contains("eliot_searchd::native_file::observe"),
            "{name} bypasses the final-handle adapter"
        );
        assert!(
            source.contains("safe_reader_adapter::read_full_file_via_kernel"),
            "{name} does not use the shared kernel translation"
        );
    }
}
