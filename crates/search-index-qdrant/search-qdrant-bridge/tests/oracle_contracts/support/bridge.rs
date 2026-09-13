use super::*;

pub(crate) fn bridge(
    receipt_capacity: usize,
) -> (QdrantBridge, CollectionRoute) {
    let supervisor = SupervisorReceipt {
        owner_epoch: OwnerEpoch::new(1).expect("fixture owner"),
        process_identity_digest: digest(2),
        artifact_digest: digest(3),
        endpoint_digest: digest(4),
    };
    // Test-double inputs for the reference bridge, not live probes.
    let capability = probe_capabilities(
        supervisor,
        digest(5),
        CapabilityProbeResults {
            topology: TopologyGates {
                authenticated_health: true,
                single_shard: true,
                signed_i64_ranges: true,
            },
            filters: FilterGates {
                missing_upper_bound_must_not: true,
                sparse_idf: true,
                independent_idf_corpus: true,
            },
            indexes: IndexGates {
                strict_mode: true,
                payload_indexes: true,
                wait_for_mutations: true,
            },
            consistency: ConsistencyGates {
                strong_ordering: true,
                exact_count_and_readback: true,
                named_sparse_vectors: true,
            },
        },
    )
    .expect("reference capability");
    let mut bridge = QdrantBridge::connect(
        BridgeEndpoint {
            endpoint_digest: digest(4),
            loopback_only: true,
        },
        AuthLeaseEvidence {
            reference_digest: digest(6),
            purpose_digest: digest(7),
            valid: true,
        },
        supervisor,
        capability,
        BridgeLimits {
            max_operation_receipts: receipt_capacity,
            ..BridgeLimits::BASELINE
        },
    )
    .expect("reference bridge");
    let route = CollectionRoute {
        generation: CollectionGenerationId::from_bytes([1; 16]),
        physical_name: opaque("oracle-contracts"),
    };
    let schema = CollectionSchema {
        named_vectors: BTreeMap::from([(
            VECTOR.to_owned(),
            VectorSchema {
                dimensions: 8,
                sparse: true,
                idf_enabled: true,
            },
        )]),
        indexed_payload_fields: EligibilityFilter::INDEXED_FIELDS
            .into_iter()
            .map(str::to_owned)
            .collect(),
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: digest(8),
    };
    bridge
        .create_candidate_collection(route.clone(), schema)
        .expect("reference collection");
    (bridge, route)
}

pub(crate) fn seeded(
    receipt_capacity: usize,
) -> (QdrantBridge, CollectionRoute) {
    let (mut bridge, route) = bridge(receipt_capacity);
    bridge
        .upsert_exact(&route, vec![point(1, 1.0)], mutation("seed", 1))
        .expect("seed point");
    (bridge, route)
}

pub(crate) fn state(
    bridge: &QdrantBridge,
    route: &CollectionRoute,
) -> Vec<PointRecord> {
    let readback = bridge
        .readback_exact(route, vec![id(1), id(2)])
        .expect("state readback");
    assert!(readback.unexpected_ids.is_empty());
    readback.points
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    Upsert,
    Close,
    Delete,
}

pub(crate) fn apply(
    bridge: &mut QdrantBridge,
    route: &CollectionRoute,
    kind: Kind,
    mutation: BridgeMutation,
) -> Result<MutationReceipt, BridgeError> {
    match kind {
        Kind::Upsert => {
            bridge.upsert_exact(route, vec![point(2, 2.0)], mutation)
        }
        Kind::Close => {
            bridge.close_exact(route, vec![id(1)], epoch(20), mutation)
        }
        Kind::Delete => bridge.delete_exact(route, vec![id(1)], mutation),
    }
}
