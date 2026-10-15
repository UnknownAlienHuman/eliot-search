//! Frozen wire-profile and semantic installation regression across the record boundary.
use super::*;
use search_contracts::{NonZeroRevision, OpaqueId};
const BATCH_PROFILE_V2: &str = "eliot.registry-batch.v2";
const BATCH_PROFILE_V3: &str = "eliot.registry-batch.v3";
const STATE_PROFILE_V2: &str = "eliot.registry-state.v2";
const STATE_PROFILE_V3: &str = "eliot.registry-state.v3";
#[cfg(test)]
mod wire_profile_tests {
    use super::*;

    #[test]
    fn decoded_batch_wire_profile_preserves_exact_v2_and_v3_bytes() {
        for encoding_profile in [BATCH_PROFILE_V2, BATCH_PROFILE_V3] {
            let corpus = OpaqueId::new("profile-retention-fixture").unwrap();
            let record = DecodedBatch::from_observed_parts(
                encoding_profile,
                if encoding_profile == BATCH_PROFILE_V2 {
                    STATE_PROFILE_V2
                } else {
                    STATE_PROFILE_V3
                },
                RegistryJournalEntry::new(
                    OpaqueId::new("profile-retention-operation").unwrap(),
                    Blake3Digest32::from_stored_bytes([1; 32]),
                    0,
                    1,
                    JournalEntryKind::BatchApply,
                    ReceiptRef::new("profile-retention-receipt").unwrap(),
                ),
                Blake3Digest32::from_stored_bytes([2; 32]),
                1,
                vec![
                    RegistryStateDelta::new(
                        RegistryStateKey::Generation(corpus.clone()),
                        None,
                        RegistryStateRecord::Generation {
                            corpus,
                            generation: NonZeroRevision::new(1).unwrap(),
                        },
                    )
                    .unwrap(),
                ],
                crate::api::DEFAULT_REGISTRY_LIMITS,
            )
            .unwrap();
            let bytes = record.encode().unwrap();
            let restored =
                decode_batch_record(&bytes, crate::api::DEFAULT_REGISTRY_LIMITS).unwrap();
            assert_eq!(restored.encoding_profile(), encoding_profile);
            assert_eq!(restored.encode().unwrap(), bytes);
            assert_eq!(restored, record);
            let state = record.deltas()[0].after();
            let key = state.key().encode().unwrap();
            let value = state
                .encode_with_profile(record.state_encoding_profile())
                .unwrap();
            let (_, actual_profile) =
                super::super::decode_registry_state_entry_profiled(&key, &value).unwrap();
            assert_eq!(actual_profile, record.state_encoding_profile());
            let mut installed =
                super::super::SourceRegistry::new(crate::api::DEFAULT_REGISTRY_LIMITS).unwrap();
            super::super::install(&mut installed, &restored).unwrap();
            assert_eq!(
                super::super::encode_existing_state(&installed, state).unwrap(),
                value,
                "expected-before bytes must retain the actual old format"
            );
            if encoding_profile == BATCH_PROFILE_V2 {
                assert_ne!(
                    value,
                    state.encode().unwrap(),
                    "current encoding would relabel v2"
                );
            }
            let operation_id = &record.entry().operation_id;
            let operations = [crate::recovery::RegistryRestoreOperation {
                operation_id,
                value: &bytes,
            }];
            let states = [crate::recovery::RegistryRestoreState {
                key: &key,
                value: &value,
            }];
            assert_eq!(
                crate::recovery::restore_registry_state(
                    1,
                    &states,
                    &operations,
                    crate::api::DEFAULT_REGISTRY_LIMITS,
                    crate::recovery::MAX_REGISTRY_RESTORE_BYTES
                )
                .unwrap_err(),
                RegistryError::MembershipNotFound,
                "this isolated codec fixture has no corpus members"
            );
            let foreign_profile = if encoding_profile == BATCH_PROFILE_V2 {
                STATE_PROFILE_V3
            } else {
                STATE_PROFILE_V2
            };
            let relabelled = state.encode_with_profile(foreign_profile).unwrap();
            let states = [crate::recovery::RegistryRestoreState {
                key: &key,
                value: &relabelled,
            }];
            assert_eq!(
                crate::recovery::restore_registry_state(
                    1,
                    &states,
                    &operations,
                    crate::api::DEFAULT_REGISTRY_LIMITS,
                    crate::recovery::MAX_REGISTRY_RESTORE_BYTES
                )
                .unwrap_err(),
                RegistryError::SnapshotInvalid,
                "semantic equality cannot relabel stored state without an operation"
            );
        }
    }
}
