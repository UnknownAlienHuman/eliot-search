//! Real-data-plane read/query operations split by operation family.
//!
//! These includes preserve the existing `RealDataPlane` public paths while
//! keeping vendor translation private to this package.

include!("queries/readback.rs");
include!("queries/count.rs");
include!("queries/scroll.rs");
include!("queries/search.rs");
