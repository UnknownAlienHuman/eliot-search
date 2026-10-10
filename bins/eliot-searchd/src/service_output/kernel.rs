//! Service-output composition behind the stable facade.

#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/indexed.rs"]
mod indexed;
#[path = "kernel/page.rs"]
mod page;
#[path = "kernel/provider.rs"]
mod provider;
#[path = "kernel/streaming.rs"]
mod streaming;

pub use codec::{MAX_RESPONSE_BYTES, json_string, write_error, write_line};
pub use indexed::emit_indexed_source;
pub use page::emit_search_page;
pub use provider::{emit_handle_expansion, emit_provider_status};
pub use streaming::emit_streaming_search;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
