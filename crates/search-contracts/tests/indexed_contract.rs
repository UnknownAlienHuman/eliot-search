use search_contracts::indexed::*;
use search_contracts::*;

fn profile() -> ProjectionProfileSetId {
    ProjectionProfileSetId::new("lexical_baseline_v1").expect("profile")
}

fn payload() -> PointPayload {
    PointPayload {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        collection_generation_id: CollectionGenerationId::from_bytes([2; 16]),
        projection_membership_id: ProjectionMembershipId::from_bytes([3; 16]),
        access_partition_id: AccessPartitionId::from_bytes([4; 16]),
        scoring_partition_id: ScoringPartitionId::from_bytes([5; 16]),
        source_id: SourceId::from_bytes([6; 16]),
        source_revision_id: SourceRevisionId::from_bytes([7; 16]),
        representation_id: RepresentationId::from_bytes([8; 16]),
        unit_id: UnitId::from_bytes([9; 16]),
        point_identity_digest_256: Blake3Digest32::from_stored_bytes([10; 32]),
        scoring_document_id: ScoringDocumentId::from_bytes([11; 16]),
        projection_profile_set_id: profile(),
        unit_kind: UnitKind::Symbol,
        modality: Modality::Code,
        language_or_format: BoundedName::new("rust").expect("format"),
        entity_kind: Some(EntityKind::Function),
        normalized_symbol_key: Some(BoundedSymbolKey::new("read_exact").expect("symbol")),
        repository_lineage_id: Some(RepositoryLineageId::from_bytes([12; 16])),
        valid_from_epoch: Epoch::new(1).expect("start"),
        valid_until_epoch_exclusive: None,
    }
}

fn population(members: &[ProjectionMembershipId], epoch: i64) -> EligibilityPopulation {
    EligibilityPopulation::new(
        InstallationIncarnationId::from_bytes([1; 16]),
        CollectionGenerationId::from_bytes([2; 16]),
        members,
        AccessPartitionId::from_bytes([4; 16]),
        ScoringPartitionId::from_bytes([5; 16]),
        profile(),
        Epoch::new(epoch).expect("visible epoch"),
    )
    .expect("population")
}

fn vectors() -> Vec<NamedVectorRequirement> {
    vec![NamedVectorRequirement {
        name: VectorName::new("lex_code_v1").expect("name"),
        requirement: VectorRequirement::Sparse {
            index_ceiling: 1024,
            idf_enabled: true,
        },
    }]
}

fn schema(generation: u8, profile: ProjectionProfileSetId) -> CollectionSchema {
    CollectionSchema::new(
        InstallationIncarnationId::from_bytes([1; 16]),
        CollectionGenerationId::from_bytes([generation; 16]),
        profile,
        VectorMode::SparseOnly,
        &vectors(),
        CollectionInvariants::BASELINE,
    )
    .expect("schema")
}

fn replace_field(value: CanonicalValue, name: &str, replacement: CanonicalValue) -> CanonicalValue {
    let CanonicalValue::Object(fields) = value else {
        panic!("closed object");
    };
    let key = CanonicalKey::new_non_empty(name).expect("field");
    let mut fields = fields;
    fields.remove(&key);
    fields.insert(key, replacement).expect("bounded fields");
    CanonicalValue::Object(fields)
}

#[test]
fn schema_decode_rejects_fixed_table_fixture_epoch_and_strictness_tampering() {
    let accepted = schema(2, profile())
        .to_canonical_value()
        .expect("schema value");
    for (name, replacement) in [
        ("payload_plan", CanonicalValue::Null),
        ("index_plan", CanonicalValue::Null),
        ("fixture_version", CanonicalValue::U64(2)),
        (
            "canonical_profile",
            CanonicalValue::Text(CanonicalText::new("unknown/v1").expect("text")),
        ),
        ("fixture_table_digest", CanonicalValue::Null),
        ("epoch_min", CanonicalValue::U64(1)),
        (
            "epoch_max",
            CanonicalValue::U64(MAX_QDRANT_EPOCH as u64 + 1),
        ),
        (
            "vector_mode",
            CanonicalValue::Text(CanonicalText::new("global").expect("text")),
        ),
        ("source_text", CanonicalValue::Null),
    ] {
        assert!(
            CollectionSchema::from_canonical_value(replace_field(
                accepted.clone(),
                name,
                replacement
            ))
            .is_err(),
            "tampered {name} must fail closed"
        );
    }
    let CanonicalValue::Object(fields) = &accepted else {
        panic!("schema object");
    };
    let invariants = fields
        .get(&CanonicalKey::new_non_empty("invariants").expect("key"))
        .expect("invariants")
        .clone();
    for name in ["strict_mode", "wait_for_mutations", "strong_ordering"] {
        let weakened = replace_field(invariants.clone(), name, CanonicalValue::Bool(false));
        assert!(
            CollectionSchema::from_canonical_value(replace_field(
                accepted.clone(),
                "invariants",
                weakened
            ))
            .is_err()
        );
    }
    for name in [
        "nodes",
        "shards",
        "replication_factor",
        "write_consistency_factor",
    ] {
        for incompatible in [0, 2] {
            let changed =
                replace_field(invariants.clone(), name, CanonicalValue::U64(incompatible));
            assert!(
                CollectionSchema::from_canonical_value(replace_field(
                    accepted.clone(),
                    "invariants",
                    changed
                ))
                .is_err()
            );
        }
    }
    let substituted = CanonicalValue::Bytes(BoundedBytes::new(vec![99; 32]).expect("digest bytes"));
    assert!(
        CollectionSchema::from_canonical_value(replace_field(
            accepted,
            "fixture_table_digest",
            substituted
        ))
        .is_err()
    );
}

#[test]
fn every_eligibility_coordinate_excludes_a_denied_candidate() {
    let admitted = payload();
    let population = population(&[admitted.projection_membership_id], 2);
    assert!(population.matches(&admitted));
    let mut denied = admitted.clone();
    denied.installation_incarnation_id = InstallationIncarnationId::from_bytes([99; 16]);
    assert!(!population.matches(&denied));
    denied = admitted.clone();
    denied.collection_generation_id = CollectionGenerationId::from_bytes([99; 16]);
    assert!(!population.matches(&denied));
    denied = admitted.clone();
    denied.projection_membership_id = ProjectionMembershipId::from_bytes([99; 16]);
    assert!(!population.matches(&denied));
    denied = admitted.clone();
    denied.access_partition_id = AccessPartitionId::from_bytes([99; 16]);
    assert!(!population.matches(&denied));
    denied = admitted.clone();
    denied.scoring_partition_id = ScoringPartitionId::from_bytes([99; 16]);
    assert!(!population.matches(&denied));
    denied = admitted;
    denied.projection_profile_set_id =
        ProjectionProfileSetId::new("different_v1").expect("profile");
    assert!(!population.matches(&denied));
}

#[test]
fn validity_edges_and_empty_generation_share_the_same_population() {
    let mut point = payload();
    let members = [point.projection_membership_id];
    assert!(!population(&members, 0).matches(&point));
    assert!(population(&members, 1).matches(&point));
    point.valid_until_epoch_exclusive = Some(Epoch::new(3).expect("end"));
    assert!(population(&members, 2).matches(&point));
    assert!(!population(&members, 3).matches(&point));
    point.valid_from_epoch = Epoch::new(MAX_QDRANT_EPOCH).expect("last");
    point.valid_until_epoch_exclusive = None;
    point.validate().expect("last epoch remains readable");
    assert!(population(&members, MAX_QDRANT_EPOCH).matches(&point));
}

#[test]
fn member_permutation_and_all_consumers_have_one_semantic_population() {
    let a = ProjectionMembershipId::from_bytes([3; 16]);
    let b = ProjectionMembershipId::from_bytes([13; 16]);
    let left = population(&[a, b], 2);
    let right = population(&[b, a], 2);
    assert_eq!(left, right);
    assert_eq!(
        left.to_canonical_value().expect("left"),
        right.to_canonical_value().expect("right")
    );
    // Each consumer receives the same closed value; no omitted/global IDF variant exists.
    let [retrieval, idf, count, scroll] = [left.predicate(); 4];
    for consumer in [retrieval, idf, count, scroll] {
        assert_eq!(consumer, &left);
        assert!(consumer.matches(&payload()));
    }
}

#[test]
fn schema_restore_recomputes_and_rejects_a_substituted_digest() {
    let accepted = schema(2, profile());
    let bytes = accepted.to_canonical_cbor().expect("encode");
    let identity = accepted.identity().expect("identity");
    let decoded =
        CollectionSchema::restore(bytes.as_slice(), *identity.as_digest()).expect("restore");
    assert_eq!(decoded, accepted);
    assert!(
        CollectionSchema::restore(
            bytes.as_slice(),
            Blake3Digest32::from_stored_bytes([99; 32])
        )
        .is_err()
    );
    assert_ne!(
        identity,
        schema(14, profile()).identity().expect("new generation")
    );
    assert_ne!(
        identity,
        schema(
            2,
            ProjectionProfileSetId::new("different_v1").expect("profile")
        )
        .identity()
        .expect("new profile")
    );
}

#[test]
fn schema_and_fixed_fixture_table_have_frozen_version_one_goldens() {
    let schema = schema(2, profile());
    assert_eq!(
        schema.identity().expect("identity").as_digest().to_string(),
        "a8ac40b877eccc2fcdd08874a9b11805198cb7b791361e6da328d345169fd39c"
    );
    let value = schema.to_canonical_value().expect("closed value");
    let CanonicalValue::Object(fields) = value else {
        panic!("schema object")
    };
    let CanonicalValue::Bytes(fixture) = fields
        .get(&CanonicalKey::new("fixture_table_digest").expect("key"))
        .expect("fixture digest")
    else {
        panic!("fixture bytes")
    };
    let expected = Blake3Digest32::parse_hex(
        "b4214c7d71b995bbeec7657a90c37f0f7de4ab759cef4450106522ab1930cf98",
    )
    .expect("fixture golden");
    assert_eq!(fixture.as_slice(), expected.as_bytes());
    let bytes = schema.to_canonical_cbor().expect("schema CBOR");
    assert_eq!(bytes.len(), 2450);
    let restored = CollectionSchema::from_canonical_cbor(bytes.as_slice()).expect("restore");
    assert_eq!(restored.to_canonical_cbor().expect("re-encode"), bytes);
}

#[test]
fn a_same_number_epoch_cannot_cross_generation_or_schema_binding() {
    let population = population(&[ProjectionMembershipId::from_bytes([3; 16])], 2);
    population
        .validate_for_schema(&schema(2, profile()))
        .expect("same binding");
    assert!(
        population
            .validate_for_schema(&schema(14, profile()))
            .is_err()
    );
    assert!(
        population
            .validate_for_schema(&schema(
                2,
                ProjectionProfileSetId::new("different_v1").expect("profile")
            ))
            .is_err()
    );
    assert_ne!(
        GenerationEpoch::new(
            CollectionGenerationId::from_bytes([2; 16]),
            Epoch::new(2).expect("epoch")
        ),
        GenerationEpoch::new(
            CollectionGenerationId::from_bytes([14; 16]),
            Epoch::new(2).expect("epoch")
        ),
    );
}

#[test]
fn payload_canonical_readback_preserves_optional_fields_and_source_identity() {
    let mut point = payload();
    let identity = point.identity().expect("identity");
    let bytes = point.to_canonical_cbor().expect("bytes");
    assert_eq!(
        PointPayload::from_canonical_cbor(bytes.as_slice()).expect("decode"),
        point
    );
    point.entity_kind = None;
    point.normalized_symbol_key = None;
    point.repository_lineage_id = None;
    let absent = point.to_canonical_cbor().expect("absent optional");
    assert_eq!(
        PointPayload::from_canonical_cbor(absent.as_slice()).expect("decode"),
        point
    );
    assert_ne!(identity, point.identity().expect("absence is load bearing"));
    point.source_revision_id = SourceRevisionId::from_bytes([15; 16]);
    assert_ne!(
        identity,
        point.identity().expect("revision is load bearing")
    );
}

#[test]
fn point_readback_requires_every_exact_named_vector_and_eligible_payload() {
    let schema = schema(2, profile());
    let population = population(&[ProjectionMembershipId::from_bytes([3; 16])], 2);
    let mut point = PointReadback {
        payload: payload(),
        vectors: vec![NamedVectorValue {
            name: VectorName::new("lex_code_v1").expect("name"),
            value: VectorValue::Sparse {
                indices: vec![0, 1023],
                values: vec![1.0, 0.5],
            },
        }],
    };
    point
        .validate(&schema, &population)
        .expect("complete exact readback");
    point.vectors.clear();
    assert!(point.validate(&schema, &population).is_err());
    point.vectors.push(NamedVectorValue {
        name: VectorName::new("unknown_v1").expect("name"),
        value: VectorValue::Sparse {
            indices: vec![0],
            values: vec![1.0],
        },
    });
    assert!(point.validate(&schema, &population).is_err());
    point.vectors[0].name = VectorName::new("lex_code_v1").expect("name");
    point.vectors[0].value = VectorValue::Sparse {
        indices: vec![1024],
        values: vec![1.0],
    };
    assert!(point.validate(&schema, &population).is_err());
    point.vectors[0].value = VectorValue::Sparse {
        indices: vec![0],
        values: vec![1.0],
    };
    point.payload.access_partition_id = AccessPartitionId::from_bytes([99; 16]);
    assert!(point.validate(&schema, &population).is_err());
}

#[test]
fn collection_and_point_readback_accept_exact_unordered_named_sets() {
    let mut requirements = vectors();
    requirements.push(NamedVectorRequirement {
        name: VectorName::new("lex_text_neutral_v1").expect("name"),
        requirement: VectorRequirement::Sparse {
            index_ceiling: 64,
            idf_enabled: true,
        },
    });
    let schema = CollectionSchema::initial(
        InstallationIncarnationId::from_bytes([1; 16]),
        CollectionGenerationId::from_bytes([2; 16]),
        profile(),
        &requirements,
    )
    .expect("two sparse named vectors");
    requirements.reverse();
    let indexes = payload_indexes()
        .map(|field| (field.name.to_owned(), field.kind))
        .collect::<Vec<_>>();
    schema
        .validate_readback(&indexes, &requirements, CollectionInvariants::BASELINE)
        .expect("provider map order is irrelevant");
    let population = population(&[ProjectionMembershipId::from_bytes([3; 16])], 2);
    let mut point = PointReadback {
        payload: payload(),
        vectors: requirements
            .iter()
            .map(|requirement| NamedVectorValue {
                name: requirement.name.clone(),
                value: VectorValue::Sparse {
                    indices: vec![0],
                    values: vec![1.0],
                },
            })
            .collect(),
    };
    point
        .validate(&schema, &population)
        .expect("exact unordered vectors");
    point.vectors[1] = point.vectors[0].clone();
    assert_eq!(
        point
            .validate(&schema, &population)
            .expect_err("duplicate cannot replace missing name")
            .kind(),
        ContractErrorKind::Duplicate
    );
    requirements[1] = requirements[0].clone();
    assert!(
        schema
            .validate_readback(&indexes, &requirements, CollectionInvariants::BASELINE)
            .is_err()
    );
}

#[test]
fn every_absent_optional_field_is_omitted_and_explicit_null_is_refused() {
    let mut point = payload();
    point.entity_kind = None;
    point.normalized_symbol_key = None;
    point.repository_lineage_id = None;
    point.valid_until_epoch_exclusive = None;
    let value = point.to_canonical_value().expect("closed payload");
    let CanonicalValue::Object(fields) = &value else {
        panic!("payload object");
    };
    for name in [
        "entity_kind",
        "normalized_symbol_key",
        "repository_lineage_id",
        "valid_until_epoch_exclusive",
    ] {
        let key = CanonicalKey::new_non_empty(name).expect("key");
        assert!(fields.get(&key).is_none(), "absent {name} must be omitted");
        let mut entries = fields
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        entries.push((key, CanonicalValue::Null));
        let with_null =
            CanonicalValue::Object(BoundedMap::from_entries(entries).expect("unique fields"));
        assert!(
            PointPayload::from_canonical_value(with_null).is_err(),
            "explicit null {name} must be rejected"
        );
    }
}

#[test]
fn point_readback_bounds_the_aggregate_of_individually_valid_vectors() {
    let requirements = [
        NamedVectorRequirement {
            name: VectorName::new("lex_code_v1").expect("name"),
            requirement: VectorRequirement::Sparse {
                index_ceiling: 65_536,
                idf_enabled: true,
            },
        },
        NamedVectorRequirement {
            name: VectorName::new("sem_fixture_v1").expect("name"),
            requirement: VectorRequirement::Dense {
                dimensions: 32_767,
                distance: DenseDistance::Cosine,
            },
        },
    ];
    let schema = CollectionSchema::new(
        InstallationIncarnationId::from_bytes([1; 16]),
        CollectionGenerationId::from_bytes([2; 16]),
        profile(),
        VectorMode::SparseWithDense,
        &requirements,
        CollectionInvariants::BASELINE,
    )
    .expect("finite schema");
    let point = PointReadback {
        payload: payload(),
        vectors: vec![
            NamedVectorValue {
                name: requirements[0].name.clone(),
                value: VectorValue::Sparse {
                    indices: (0..32_770).collect(),
                    values: vec![1.0; 32_770],
                },
            },
            NamedVectorValue {
                name: requirements[1].name.clone(),
                value: VectorValue::Dense(vec![1.0; 32_767]),
            },
        ],
    };
    for (actual, required) in point.vectors.iter().zip(&requirements) {
        actual
            .value
            .validate(&required.requirement)
            .expect("individually valid");
    }
    let population = population(&[ProjectionMembershipId::from_bytes([3; 16])], 2);
    assert_eq!(
        point
            .validate(&schema, &population)
            .expect_err("aggregate limit")
            .field(),
        "point_readback_values"
    );
}
