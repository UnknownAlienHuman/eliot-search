// Real-data-plane read/query operations split by operation family.
//!
//! These includes preserve the existing `RealDataPlane` public paths while
//! keeping vendor translation private to this package.

use super::{
    BTreeMap, BTreeSet, BoundedPointReadback, BridgeError, CandidateNomination,
    CollectionRoute, CountPoints, EligibilityFilter, ExactCount, GetPoints, IdfParams,
    OpContext, OperationBudget, PointRecord, QdrantPointId, Query, QueryPoints,
    RealDataPlane, ScrollPage, ScrollPoints, SearchParams, VectorInput, base_filter,
    bridge_point_id, collection_name, decode_payload, decode_point, ensure_filter_indexes,
    map_read_error, validate_exact_ids, validate_filter_for_route, validate_query_vector,
    vendor_point_id,
};

include!("queries/readback.rs");
include!("queries/count.rs");
include!("queries/scroll.rs");
include!("queries/search.rs");
