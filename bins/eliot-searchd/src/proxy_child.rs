//! Bounded pipe execution for the one DIRECT child owned by the proxy.
//!
//! Process lifetime, queue state, pipe forwarding, deadline accounting and
//! regression coverage live in bounded private owners below this facade.

use super::exchange::{Reply, forward_reply};
use super::{Terminal, MAX_PROXY_COMMAND_BYTES};

mod lifecycle;
mod model;
mod pipe;
mod spec;
mod time;
mod worker;

pub(super) use lifecycle::ChildIo;
pub(super) use pipe::write_admitted_line;
pub(super) use spec::ChildLimits;

#[cfg(test)]
mod tests;
