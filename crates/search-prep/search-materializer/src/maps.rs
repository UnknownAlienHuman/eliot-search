//! Coordinate and loss map production and bundle validation.
//!
//! Data contracts, deterministic builders and independent validation are
//! separated so map construction cannot bypass the validation boundary.

mod build;
mod model;
mod validate;

pub use build::{build_coordinate_map, build_loss_map};
pub use model::{
    COORDINATE_MAP_VERSION, CoordinateMap, CoordinateSegment, LossKind, LossMap, LossRecord,
    MapBundle, MapIdentities, MapValidationReceipt, SegmentRelation,
};
pub use validate::{validate_coordinate_map, validate_map_bundle};

#[cfg(test)]
mod tests;
