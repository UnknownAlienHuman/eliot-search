// Private vendor error mapping and schema verification by operation class.

use super::{
    BridgeError, CODE_ALREADY_EXISTS, CODE_FAILED_PRECONDITION, CODE_INVALID_ARGUMENT,
    CODE_NOT_FOUND, CODE_OUT_OF_RANGE, CODE_PERMISSION_DENIED, CODE_RESOURCE_EXHAUSTED,
    CODE_UNAUTHENTICATED, CollectionInfo, CollectionSchema, Modifier, PointPayload,
    payload_index_specs, vendor_schema_type,
};

include!("errors_schema/read.rs");
include!("errors_schema/mutation.rs");
include!("errors_schema/create.rs");
include!("errors_schema/verify.rs");
