//! Service-output composition behind the stable facade.

mod codec;
mod indexed;
mod page;
mod provider;
mod streaming;

pub use codec::{MAX_RESPONSE_BYTES, json_string, write_error, write_line};
pub use indexed::emit_indexed_source;
pub use page::emit_search_page;
pub use provider::{emit_handle_expansion, emit_provider_status};
pub use streaming::emit_streaming_search;

#[cfg(test)]
mod tests;
