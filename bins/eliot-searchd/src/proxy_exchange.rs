//! A child stream is reusable only after its entire response is consumed.
//!
//! Outcome fencing, terminal-frame parsing, exact forwarding and regression
//! coverage live in bounded private owners below this facade.

#[path = "proxy_exchange/fence.rs"]
mod fence;
#[path = "proxy_exchange/forward.rs"]
mod forward;
#[path = "proxy_exchange/parser.rs"]
mod parser;
#[path = "proxy_exchange/reply.rs"]
mod reply;

pub(super) use fence::ExchangeFence;
pub(super) use forward::forward_reply;
pub(super) use parser::event_name;
pub(super) use reply::Reply;

#[cfg(test)]
#[path = "proxy_exchange/tests.rs"]
mod tests;
