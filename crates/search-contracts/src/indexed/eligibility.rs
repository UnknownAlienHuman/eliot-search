//! S10.3 eligibility population: one closed, bounded, post-fence value.
//!
//! This is the single compiled population shared by retrieval, `idf.corpus`,
//! count and scroll consumers. It carries no provider type, no global or
//! omitted scope variant, and no source, publication or access authority.
//! Restrictive fences are compiled into the membership set before this value
//! is built; a bare epoch is never cross-generation authority.

use crate::{
    AccessPartitionId, BoundedSet, CanonicalValue, CollectionGenerationId, ContractError,
    ContractErrorKind, Epoch, InstallationIncarnationId, MAX_COLLECTION_ITEMS,
    ProjectionMembershipId, ProjectionProfileSetId, ScoringPartitionId,
};

use super::codec::{array, error, object, text};
use super::collection::CollectionSchema;
use super::payload::PointPayload;

const FIELD_INCARNATION: &str = "installation_incarnation_id";
const FIELD_GENERATION: &str = "collection_generation_id";
const FIELD_MEMBERSHIPS: &str = "allowed_projection_memberships";
const FIELD_ACCESS: &str = "access_partition_id";
const FIELD_SCORING: &str = "scoring_partition_id";
const FIELD_PROFILE: &str = "projection_profile_set_id";
const FIELD_VISIBLE_EPOCH: &str = "visible_epoch";

/// A collection generation paired with its visible epoch.
///
/// Equality covers both coordinates. `Ord` is deliberately absent: ordering
/// one generation's epoch against another would imply a global clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationEpoch {
    collection_generation_id: CollectionGenerationId,
    visible_epoch: Epoch,
}

impl GenerationEpoch {
    #[must_use]
    pub const fn new(
        collection_generation_id: CollectionGenerationId,
        visible_epoch: Epoch,
    ) -> Self {
        Self {
            collection_generation_id,
            visible_epoch,
        }
    }

    #[must_use]
    pub const fn collection_generation_id(&self) -> CollectionGenerationId {
        self.collection_generation_id
    }

    #[must_use]
    pub const fn visible_epoch(&self) -> Epoch {
        self.visible_epoch
    }
}

/// One closed S10.3 eligibility population.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilityPopulation {
    installation_incarnation_id: InstallationIncarnationId,
    collection_generation_id: CollectionGenerationId,
    allowed_projection_memberships: BoundedSet<ProjectionMembershipId, MAX_COLLECTION_ITEMS>,
    access_partition_id: AccessPartitionId,
    scoring_partition_id: ScoringPartitionId,
    projection_profile_set_id: ProjectionProfileSetId,
    visible_epoch: Epoch,
}

impl EligibilityPopulation {
    /// Builds one population, refusing an unusable membership set before any
    /// owned allocation: empty, duplicated, or over the finite ceiling.
    pub fn new(
        installation_incarnation_id: InstallationIncarnationId,
        collection_generation_id: CollectionGenerationId,
        allowed_projection_memberships: &[ProjectionMembershipId],
        access_partition_id: AccessPartitionId,
        scoring_partition_id: ScoringPartitionId,
        projection_profile_set_id: ProjectionProfileSetId,
        visible_epoch: Epoch,
    ) -> Result<Self, ContractError> {
        if allowed_projection_memberships.is_empty() {
            return Err(error(ContractErrorKind::Empty, FIELD_MEMBERSHIPS));
        }
        if allowed_projection_memberships.len() > MAX_COLLECTION_ITEMS {
            return Err(error(ContractErrorKind::TooManyItems, FIELD_MEMBERSHIPS));
        }
        // A repeated identity is a caller bug, not a wider population.
        for (position, membership) in allowed_projection_memberships.iter().enumerate() {
            if allowed_projection_memberships[..position].contains(membership) {
                return Err(error(ContractErrorKind::Duplicate, FIELD_MEMBERSHIPS));
            }
        }
        // Sorted storage makes insertion order irrelevant to identity.
        let mut sorted = allowed_projection_memberships.to_vec();
        sorted.sort_unstable();
        let memberships = BoundedSet::from_items(sorted)
            .map_err(|_| error(ContractErrorKind::TooManyItems, FIELD_MEMBERSHIPS))?;
        Ok(Self {
            installation_incarnation_id,
            collection_generation_id,
            allowed_projection_memberships: memberships,
            access_partition_id,
            scoring_partition_id,
            projection_profile_set_id,
            visible_epoch,
        })
    }

    #[must_use]
    pub const fn installation_incarnation_id(&self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    #[must_use]
    pub const fn collection_generation_id(&self) -> CollectionGenerationId {
        self.collection_generation_id
    }

    #[must_use]
    pub const fn access_partition_id(&self) -> AccessPartitionId {
        self.access_partition_id
    }

    #[must_use]
    pub const fn scoring_partition_id(&self) -> ScoringPartitionId {
        self.scoring_partition_id
    }

    #[must_use]
    pub const fn projection_profile_set_id(&self) -> &ProjectionProfileSetId {
        &self.projection_profile_set_id
    }

    #[must_use]
    pub const fn visible_epoch(&self) -> Epoch {
        self.visible_epoch
    }

    #[must_use]
    pub fn allowed_projection_memberships(&self) -> Vec<ProjectionMembershipId> {
        self.allowed_projection_memberships
            .iter()
            .copied()
            .collect()
    }

    #[must_use]
    pub const fn generation_epoch(&self) -> GenerationEpoch {
        GenerationEpoch {
            collection_generation_id: self.collection_generation_id,
            visible_epoch: self.visible_epoch,
        }
    }

    /// Whether one payload point is inside this population.
    ///
    /// The payload is validated first, so a malformed point never participates.
    /// Epoch zero is the empty initial generation and admits no point. The
    /// generation is compared before any epoch, so a bare epoch is never read
    /// as a global clock. The interval is half-open: `valid_from <= visible`,
    /// with an absent or strictly later exclusive upper bound.
    #[must_use]
    pub fn matches(&self, payload: &PointPayload) -> bool {
        if payload.validate().is_err() {
            return false;
        }
        if self.visible_epoch.get() == 0 {
            return false;
        }
        if payload.collection_generation_id != self.collection_generation_id {
            return false;
        }
        if payload.installation_incarnation_id != self.installation_incarnation_id
            || payload.access_partition_id != self.access_partition_id
            || payload.scoring_partition_id != self.scoring_partition_id
            || payload.projection_profile_set_id != self.projection_profile_set_id
        {
            return false;
        }
        if !self
            .allowed_projection_memberships
            .contains(&payload.projection_membership_id)
        {
            return false;
        }
        if payload.valid_from_epoch.get() > self.visible_epoch.get() {
            return false;
        }
        payload
            .valid_until_epoch_exclusive
            .is_none_or(|until| self.visible_epoch.get() < until.get())
    }

    /// The one predicate handed to every retrieval, IDF, count and scroll
    /// consumer. Returning `&Self` makes a use-case variant unrepresentable.
    #[must_use]
    pub const fn predicate(&self) -> &Self {
        self
    }

    /// Exact schema binding: incarnation, generation and profile must agree.
    pub fn validate_for_schema(&self, schema: &CollectionSchema) -> Result<(), ContractError> {
        if &self.installation_incarnation_id != schema.installation_incarnation_id() {
            return Err(error(ContractErrorKind::FamilyMismatch, FIELD_INCARNATION));
        }
        if &self.collection_generation_id != schema.collection_generation_id() {
            return Err(error(ContractErrorKind::FamilyMismatch, FIELD_GENERATION));
        }
        if self.projection_profile_set_id() != schema.projection_profile_set_id() {
            return Err(error(ContractErrorKind::FamilyMismatch, FIELD_PROFILE));
        }
        Ok(())
    }

    /// Closed canonical value. Sorted memberships make insertion order
    /// irrelevant to the encoded bytes.
    pub fn to_canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        let members = self
            .allowed_projection_memberships
            .iter()
            .map(|membership| text(&membership.to_string()))
            .collect::<Result<Vec<_>, ContractError>>()?;
        object([
            (
                FIELD_INCARNATION,
                text(&self.installation_incarnation_id.to_string())?,
            ),
            (
                FIELD_GENERATION,
                text(&self.collection_generation_id.to_string())?,
            ),
            (FIELD_MEMBERSHIPS, array(members)?),
            (FIELD_ACCESS, text(&self.access_partition_id.to_string())?),
            (FIELD_SCORING, text(&self.scoring_partition_id.to_string())?),
            (
                FIELD_PROFILE,
                text(self.projection_profile_set_id.as_str())?,
            ),
            (
                FIELD_VISIBLE_EPOCH,
                CanonicalValue::U64(
                    u64::try_from(self.visible_epoch.get()).map_err(|_| {
                        error(ContractErrorKind::EpochOutOfRange, FIELD_VISIBLE_EPOCH)
                    })?,
                ),
            ),
        ])
    }

    /// Closed canonical decode. Every declared field is removed and any
    /// residual field fails closed rather than being ignored.
    pub fn from_canonical_value(value: CanonicalValue) -> Result<Self, ContractError> {
        let mut fields = crate::ClosedCanonicalObject::from_value(value, FIELD_VISIBLE_EPOCH)?;
        // Bound the membership set before any owned allocation: an oversize or
        // empty set is refused on the borrowed slice, never after cloning it.
        let raw_members = super::codec::decode_array(
            fields.take_required(FIELD_MEMBERSHIPS)?,
            FIELD_MEMBERSHIPS,
        )?;
        if raw_members.is_empty() {
            return Err(error(ContractErrorKind::Empty, FIELD_MEMBERSHIPS));
        }
        if raw_members.len() > MAX_COLLECTION_ITEMS {
            return Err(error(ContractErrorKind::TooManyItems, FIELD_MEMBERSHIPS));
        }
        let population = Self::new(
            InstallationIncarnationId::parse(&super::codec::take_text(
                &mut fields,
                FIELD_INCARNATION,
            )?)
            .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_INCARNATION))?,
            CollectionGenerationId::parse(&super::codec::take_text(&mut fields, FIELD_GENERATION)?)
                .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_GENERATION))?,
            &parse_memberships(&raw_members, FIELD_MEMBERSHIPS)?,
            AccessPartitionId::parse(&super::codec::take_text(&mut fields, FIELD_ACCESS)?)
                .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_ACCESS))?,
            ScoringPartitionId::parse(&super::codec::take_text(&mut fields, FIELD_SCORING)?)
                .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_SCORING))?,
            ProjectionProfileSetId::new(super::codec::take_text(&mut fields, FIELD_PROFILE)?)?,
            super::codec::epoch(
                fields.take_required(FIELD_VISIBLE_EPOCH)?,
                FIELD_VISIBLE_EPOCH,
            )?,
        )?;
        fields.finish()?;
        Ok(population)
    }
}

fn parse_memberships(
    values: &[CanonicalValue],
    field: &'static str,
) -> Result<Vec<ProjectionMembershipId>, ContractError> {
    values
        .iter()
        .map(|value| {
            // Borrow the typed text; a wrong-typed value is refused without
            // cloning any subtree.
            match value {
                CanonicalValue::Text(text) => ProjectionMembershipId::parse(text.as_str())
                    .map_err(|_| error(ContractErrorKind::MalformedPayload, field)),
                _ => Err(error(ContractErrorKind::MalformedPayload, field)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalKey;

    fn member(byte: u8) -> ProjectionMembershipId {
        ProjectionMembershipId::from_bytes([byte; 16])
    }

    fn members(count: usize) -> Vec<ProjectionMembershipId> {
        // Full-width counter identities remain distinct beyond 256 entries.
        (0..count)
            .map(|index| ProjectionMembershipId::from_bytes((index as u128).to_be_bytes()))
            .collect()
    }

    fn try_population(
        members: &[ProjectionMembershipId],
        epoch: i64,
    ) -> Result<EligibilityPopulation, ContractError> {
        EligibilityPopulation::new(
            InstallationIncarnationId::from_bytes([1; 16]),
            CollectionGenerationId::from_bytes([2; 16]),
            members,
            AccessPartitionId::from_bytes([4; 16]),
            ScoringPartitionId::from_bytes([5; 16]),
            ProjectionProfileSetId::new("lexical_baseline_v1").expect("profile"),
            Epoch::new(epoch).expect("visible epoch"),
        )
    }

    fn population(members: &[ProjectionMembershipId], epoch: i64) -> EligibilityPopulation {
        try_population(members, epoch).expect("population")
    }

    fn value() -> CanonicalValue {
        let single = [member(3)];
        population(&single, 2).to_canonical_value().expect("value")
    }

    #[test]
    fn constructor_refuses_empty_duplicate_and_oversize_member_sets() {
        // Empty is an invalid filter, never a zero-population match.
        let error = EligibilityPopulation::new(
            InstallationIncarnationId::from_bytes([1; 16]),
            CollectionGenerationId::from_bytes([2; 16]),
            &[],
            AccessPartitionId::from_bytes([4; 16]),
            ScoringPartitionId::from_bytes([5; 16]),
            ProjectionProfileSetId::new("lexical_baseline_v1").expect("profile"),
            Epoch::new(2).expect("visible epoch"),
        )
        .expect_err("empty membership set");
        assert_eq!(error.field(), FIELD_MEMBERSHIPS);
        assert_eq!(error.kind(), ContractErrorKind::Empty);

        // A repeated identity is a caller bug, not a wider population.
        let duplicate = [member(7), member(7)];
        let error = try_population(&duplicate, 2).expect_err("duplicate membership");
        assert_eq!(error.field(), FIELD_MEMBERSHIPS);
        assert_eq!(error.kind(), ContractErrorKind::Duplicate);

        // The ceiling is inclusive at the bound and refuses one past it.
        let at_limit = members(MAX_COLLECTION_ITEMS);
        assert_eq!(at_limit.len(), MAX_COLLECTION_ITEMS);
        let accepted = population(&at_limit, 2);
        assert_eq!(
            accepted.allowed_projection_memberships().len(),
            MAX_COLLECTION_ITEMS
        );
        let over = members(MAX_COLLECTION_ITEMS + 1);
        assert_eq!(over.len(), MAX_COLLECTION_ITEMS + 1);
        let error = try_population(&over, 2).expect_err("oversize membership set");
        assert_eq!(error.field(), FIELD_MEMBERSHIPS);
        assert_eq!(error.kind(), ContractErrorKind::TooManyItems);
    }

    #[test]
    fn decode_refuses_missing_extra_and_wrong_typed_fields() {
        fn without(name: &str) -> CanonicalValue {
            let CanonicalValue::Object(mut fields) = value() else {
                panic!("object")
            };
            fields.remove(&CanonicalKey::new(name).expect("key"));
            CanonicalValue::Object(fields)
        }
        let good = value();
        // Parity first, so every later failure is attributable to the edit.
        let decoded =
            EligibilityPopulation::from_canonical_value(good).expect("encoded value decodes");
        assert_eq!(decoded.to_canonical_value().expect("re-encode"), value());

        // A missing declared field fails closed rather than defaulting.
        let error = EligibilityPopulation::from_canonical_value(without(FIELD_MEMBERSHIPS))
            .expect_err("missing memberships");
        assert_eq!(error.field(), FIELD_MEMBERSHIPS);

        let error = EligibilityPopulation::from_canonical_value(without(FIELD_VISIBLE_EPOCH))
            .expect_err("missing visible epoch");
        assert_eq!(error.field(), FIELD_VISIBLE_EPOCH);

        let error = EligibilityPopulation::from_canonical_value(without(FIELD_PROFILE))
            .expect_err("missing profile");
        assert_eq!(error.field(), FIELD_PROFILE);

        // An extra residual field fails closed after the declared ones.
        let CanonicalValue::Object(mut extra) = value() else {
            panic!("object")
        };
        extra
            .insert(
                CanonicalKey::new("source_text").expect("key"),
                CanonicalValue::Null,
            )
            .expect("extra");
        let error = EligibilityPopulation::from_canonical_value(CanonicalValue::Object(extra))
            .expect_err("unknown residual field");
        assert_eq!(error.kind(), ContractErrorKind::UnknownField);

        // A wrong-typed declared field is refused by the scalar decoder.
        let error = EligibilityPopulation::from_canonical_value(
            super::super::codec::object([
                (FIELD_MEMBERSHIPS, CanonicalValue::U64(7)),
                (FIELD_VISIBLE_EPOCH, CanonicalValue::U64(2)),
            ])
            .expect("object"),
        )
        .expect_err("memberships as integer");
        assert_eq!(error.field(), FIELD_MEMBERSHIPS);

        // The closed object rejects a non-object value outright.
        let error = EligibilityPopulation::from_canonical_value(CanonicalValue::U64(1))
            .expect_err("not an object");
        assert_eq!(error.kind(), ContractErrorKind::InvalidTaggedVariant);
    }

    #[test]
    fn member_order_permutation_is_one_population() {
        let first = member(11);
        let second = member(22);
        let forward = population(&[first, second], 2);
        let reversed = population(&[second, first], 2);
        assert_eq!(forward, reversed);
        assert_eq!(
            forward.to_canonical_value().expect("forward"),
            reversed.to_canonical_value().expect("reversed"),
        );
        // The sorted set is what makes insertion order irrelevant.
        assert_eq!(
            forward.allowed_projection_memberships(),
            vec![first, second]
        );
        assert_eq!(
            reversed.allowed_projection_memberships(),
            vec![first, second]
        );
    }

    #[test]
    fn canonical_parity_round_trips_every_field() {
        let encoded = value();
        let decoded = EligibilityPopulation::from_canonical_value(encoded).expect("decode");
        assert_eq!(decoded.to_canonical_value().expect("encode"), value());
        assert_eq!(decoded, population(&[member(3)], 2));
        assert_eq!(decoded.visible_epoch(), Epoch::new(2).expect("epoch"));
        assert_eq!(
            decoded.projection_profile_set_id().as_str(),
            "lexical_baseline_v1"
        );
        let CanonicalValue::Object(fields) = decoded.to_canonical_value().expect("encode") else {
            panic!("object")
        };
        assert!(
            fields
                .get(&CanonicalKey::new(FIELD_PROFILE).expect("key"))
                .is_some()
        );
    }
}
