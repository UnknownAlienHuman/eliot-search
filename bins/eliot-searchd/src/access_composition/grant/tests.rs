use super::*;
use search_contracts::{CorpusId, ReferencePortfolioId};

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("timestamp")
}

fn profile(value: &str) -> ProfileId {
    ProfileId::new(value).expect("profile")
}

fn membership(byte: u8) -> SourceMembershipId {
    SourceMembershipId::from_bytes([byte; 16])
}

fn set<T: Ord, const LIMIT: usize>(
    values: impl IntoIterator<Item = T>,
) -> BoundedSet<T, LIMIT> {
    BoundedSet::from_items(values).expect("bounded set")
}

fn policy() -> AuthoritativeGrantPolicy {
    AuthoritativeGrantPolicy {
        binding_id: BindingId::from_bytes([1; 16]),
        binding_generation: 4,
        policy_generation: 7,
        installation_id: InstallationId::from_bytes([2; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
        principal_opaque_id: OpaqueId::new("principal").expect("principal"),
        client_scope_ref: OpaqueRef::new("client-scope").expect("scope"),
        scope_domain_id: ScopeDomainId::from_bytes([4; 16]),
        allowed_membership_ids: set([membership(10), membership(11)]),
        allowed_corpus_or_portfolio_ids: set([
            CorpusOrPortfolioId::Corpus(CorpusId::from_bytes([20; 16])),
            CorpusOrPortfolioId::Portfolio(ReferencePortfolioId::from_bytes([21; 16])),
        ]),
        reference_portfolio_revision: Some(PortfolioRevision::new(8)),
        allowed_access_partitions: set([
            AccessPartitionId::from_bytes([30; 16]),
            AccessPartitionId::from_bytes([31; 16]),
        ]),
        allowed_modalities: set([Modality::Code, Modality::Text]),
        permitted_recipe_families: set([RecipeIdV1::Locate, RecipeIdV1::FindText]),
        allowed_budget_classes: set([profile("interactive"), profile("verification")]),
        sensitivity_ceiling: SensitivityClass::Confidential,
        disclosure_ceiling: DisclosureCeiling::NamedClient,
        source_read_permission: true,
        exact_scan_permission: true,
        issued_boot_id: OpaqueId::new("boot").expect("boot"),
        revocation_generation: 9,
        maximum_ttl_ms: 60_000,
    }
}

fn request() -> StandaloneGrantRequest {
    StandaloneGrantRequest {
        operation_id: OpaqueId::new("operation-1").expect("operation"),
        binding_id: BindingId::from_bytes([1; 16]),
        expected_binding_generation: 4,
        expected_policy_generation: 7,
        requested_membership_ids: set([membership(10)]),
        requested_corpus_or_portfolio_ids: set([CorpusOrPortfolioId::Corpus(
            CorpusId::from_bytes([20; 16]),
        )]),
        requested_access_partitions: set([AccessPartitionId::from_bytes([30; 16])]),
        requested_modalities: set([Modality::Code]),
        requested_recipe_families: set([RecipeIdV1::Locate]),
        requested_budget_class: profile("interactive"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    }
}

#[derive(Default)]
struct FakeIssuer {
    retained: Option<(StandaloneGrantTemplate, StandaloneGrantMaterial)>,
    calls: usize,
    corrupt_binding: bool,
}

impl StandaloneGrantIssuer for FakeIssuer {
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        self.calls += 1;
        if let Some((retained_template, material)) = &self.retained {
            if retained_template.operation_id == template.operation_id {
                return if retained_template == template {
                    Ok(material.clone())
                } else {
                    Err(GrantIssuerError::OperationConflict)
                };
            }
        }
        let material = StandaloneGrantMaterial {
            operation_id: template.operation_id.clone(),
            binding_id: if self.corrupt_binding {
                BindingId::from_bytes([99; 16])
            } else {
                template.binding_id
            },
            binding_generation: template.binding_generation,
            policy_generation: template.policy_generation,
            grant_id: GrantId::from_bytes([40; 16]),
            nonce: OpaqueId::new("nonce").expect("nonce"),
            issued_at: timestamp("2026-09-15T10:00:00.000000Z"),
            expires_at: timestamp("2026-09-15T10:00:30.000000Z"),
            effective_ttl_ms: template.requested_ttl_ms,
        };
        self.retained = Some((template.clone(), material.clone()));
        Ok(material)
    }
}

#[test]
fn minted_claims_are_exactly_the_requested_authorized_subset() {
    let policy = policy();
    let request = request();
    let mut issuer = FakeIssuer::default();

    let grant = mint_standalone_grant(&mut issuer, &request, &policy).expect("grant minted");

    assert_eq!(issuer.calls, 1);
    assert_eq!(grant.binding_id, policy.binding_id);
    assert_eq!(
        &grant.allowed_membership_ids,
        &request.requested_membership_ids
    );
    assert_eq!(
        &grant.allowed_corpus_or_portfolio_ids,
        &request.requested_corpus_or_portfolio_ids
    );
    assert_eq!(
        &grant.allowed_access_partitions,
        &request.requested_access_partitions
    );
    assert_eq!(&grant.allowed_modalities, &request.requested_modalities);
    assert_eq!(
        &grant.permitted_recipe_families,
        &request.requested_recipe_families
    );
    assert_eq!(
        &grant.maximum_budget_class,
        &request.requested_budget_class
    );
    assert_eq!(
        grant.sensitivity_ceiling,
        request.requested_sensitivity_ceiling
    );
    assert_eq!(
        grant.disclosure_ceiling,
        request.requested_disclosure_ceiling
    );
    assert!(grant.source_read_permission);
    assert!(!grant.exact_scan_permission);
}

#[test]
fn foreign_scope_fails_before_the_issuer_runs() {
    let policy = policy();
    let mut request = request();
    request.requested_membership_ids = set([membership(99)]);
    let mut issuer = FakeIssuer::default();

    assert_eq!(
        mint_standalone_grant(&mut issuer, &request, &policy),
        Err(GrantMintError::RequestedScopeUnauthorized)
    );
    assert_eq!(issuer.calls, 0);
}

#[test]
fn requested_ceilings_and_permissions_never_widen_policy() {
    let policy = policy();
    let mut request = request();
    request.requested_sensitivity_ceiling = SensitivityClass::SecretCandidate;
    let mut issuer = FakeIssuer::default();
    assert_eq!(
        mint_standalone_grant(&mut issuer, &request, &policy),
        Err(GrantMintError::RequestedCeilingWidening)
    );
    assert_eq!(issuer.calls, 0);

    let mut policy = policy();
    policy.exact_scan_permission = false;
    let mut request = request();
    request.requested_exact_scan_permission = true;
    let mut issuer = FakeIssuer::default();
    assert_eq!(
        mint_standalone_grant(&mut issuer, &request, &policy),
        Err(GrantMintError::RequestedCeilingWidening)
    );
    assert_eq!(issuer.calls, 0);
}

#[test]
fn equal_operation_reconstructs_and_conflicting_input_is_rejected() {
    let policy = policy();
    let request = request();
    let mut issuer = FakeIssuer::default();

    let first = mint_standalone_grant(&mut issuer, &request, &policy).expect("first");
    let second = mint_standalone_grant(&mut issuer, &request, &policy).expect("replay");
    assert_eq!(first, second);

    let mut conflict = request;
    conflict.requested_ttl_ms = 10_000;
    assert_eq!(
        mint_standalone_grant(&mut issuer, &conflict, &policy),
        Err(GrantMintError::IssuerOperationConflict)
    );
}

#[test]
fn stale_generation_and_foreign_receipt_fail_closed() {
    let policy = policy();
    let mut stale = request();
    stale.expected_policy_generation += 1;
    let mut issuer = FakeIssuer::default();
    assert_eq!(
        mint_standalone_grant(&mut issuer, &stale, &policy),
        Err(GrantMintError::PolicyGenerationStale)
    );
    assert_eq!(issuer.calls, 0);

    let mut issuer = FakeIssuer {
        corrupt_binding: true,
        ..FakeIssuer::default()
    };
    assert_eq!(
        mint_standalone_grant(&mut issuer, &request(), &policy),
        Err(GrantMintError::IssuerReceiptMismatch)
    );
}
