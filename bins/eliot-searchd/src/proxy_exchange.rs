//! A child stream is reusable only after its entire response is consumed.
//!
//! Outcome fencing, terminal-frame parsing, exact forwarding and regression
//! coverage live in bounded private owners below this facade.

mod fence;
mod forward;
mod parser;
mod reply;

pub(super) use fence::ExchangeFence;
pub(super) use forward::forward_reply;
pub(super) use parser::event_name;
pub(super) use reply::Reply;

#[cfg(test)]
mod tests;
