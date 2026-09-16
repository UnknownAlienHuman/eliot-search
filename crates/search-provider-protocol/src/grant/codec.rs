//! Strict canonical codec for [`StandaloneGrantRequestV1`](super::StandaloneGrantRequestV1).

mod cursor;
mod decode;
mod encode;

pub use decode::decode_standalone_grant_request;
pub use encode::encode_standalone_grant_request;
