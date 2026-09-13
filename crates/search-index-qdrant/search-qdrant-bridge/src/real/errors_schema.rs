//! Private vendor error mapping and schema verification by operation class.

include!("errors_schema/read.rs");
include!("errors_schema/mutation.rs");
include!("errors_schema/create.rs");
include!("errors_schema/verify.rs");
