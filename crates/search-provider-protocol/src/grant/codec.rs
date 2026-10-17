//! Strict canonical codec for [`StandaloneGrantRequestV1`](super::StandaloneGrantRequestV1).

mod cursor;
mod decode;
mod encode;

pub use decode::{decode_standalone_grant_request, decode_standalone_grant_request_v2};
pub use encode::{encode_standalone_grant_request, encode_standalone_grant_request_v2};
