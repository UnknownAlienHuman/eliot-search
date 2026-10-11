//! Private numerical parameters shared by legacy and immutable code profiles.

use super::profile::{
    CollisionPolicy, DocumentTfWeighting, IdfMode, QueryTfWeighting, SparseLimits, SparseProfile,
};
use search_contracts::{BoundedList, CanonicalValue, ContractError};

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct NumericalParameters {
    pub(crate) index_space: u32,
    pub(crate) hash_seed: u64,
    pub(crate) collision_policy: CollisionPolicy,
    pub(crate) maximum_collision_rate_ppm: u32,
    pub(crate) document_tf: DocumentTfWeighting,
    pub(crate) query_tf: QueryTfWeighting,
    pub(crate) idf_mode: IdfMode,
}

impl NumericalParameters {
    pub(crate) const fn from_profile(profile: &SparseProfile) -> Self {
        Self {
            index_space: profile.index_space,
            hash_seed: profile.hash_seed,
            collision_policy: profile.collision_policy,
            maximum_collision_rate_ppm: profile.maximum_collision_rate_ppm,
            document_tf: profile.document_tf,
            query_tf: profile.query_tf,
            idf_mode: profile.idf_mode,
        }
    }

    pub(crate) const CODE_IDENTIFIERS: Self = Self {
        index_space: 1_048_576,
        hash_seed: 0x9e37_79b9_7f4a_7c15,
        collision_policy: CollisionPolicy::MergeMeasured,
        maximum_collision_rate_ppm: 10_000,
        document_tf: DocumentTfWeighting::Raw,
        query_tf: QueryTfWeighting::Binary,
        idf_mode: IdfMode::DelegatedToQdrant,
    };

    pub(crate) fn canonical(self) -> Result<CanonicalValue, ContractError> {
        let document_tf = match self.document_tf {
            DocumentTfWeighting::Raw => vec![CanonicalValue::U64(0)],
            DocumentTfWeighting::Logarithmic => vec![CanonicalValue::U64(1)],
            DocumentTfWeighting::Bm25 { k1, b } => vec![
                CanonicalValue::U64(2),
                CanonicalValue::U64(k1.to_bits()),
                CanonicalValue::U64(b.to_bits()),
            ],
        };
        let collision = match self.collision_policy {
            CollisionPolicy::Reject => 0,
            CollisionPolicy::MergeMeasured => 1,
        };
        let query_tf = match self.query_tf {
            QueryTfWeighting::Binary => 0,
            QueryTfWeighting::Raw => 1,
            QueryTfWeighting::Logarithmic => 2,
        };
        let idf = match self.idf_mode {
            IdfMode::None => 0,
            IdfMode::DelegatedToQdrant => 1,
            IdfMode::FrozenLocal => 2,
        };
        let hard = SparseLimits::BASELINE;
        BoundedList::new(vec![
            CanonicalValue::U64(u64::from(self.index_space)),
            CanonicalValue::U64(self.hash_seed),
            CanonicalValue::U64(collision),
            CanonicalValue::U64(u64::from(self.maximum_collision_rate_ppm)),
            CanonicalValue::Array(BoundedList::new(document_tf)?),
            CanonicalValue::U64(query_tf),
            CanonicalValue::U64(idf),
            CanonicalValue::Bool(self.idf_mode == IdfMode::DelegatedToQdrant),
            CanonicalValue::U64(hard.max_features as u64),
            CanonicalValue::U64(hard.max_terms_per_index as u64),
            CanonicalValue::U64(hard.max_vector_values as u64),
            CanonicalValue::U64(hard.max_collision_pairs as u64),
        ])
        .map(CanonicalValue::Array)
    }
}
