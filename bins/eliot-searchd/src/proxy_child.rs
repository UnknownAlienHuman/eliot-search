//! Bounded pipe execution for the one DIRECT child owned by the proxy.
//!
//! Process lifetime, queue state, pipe forwarding, deadline accounting and
//! regression coverage live in bounded private owners below this facade.

use super::exchange::{Reply, forward_reply};
use super::{Terminal, MAX_PROXY_COMMAND_BYTES};

#[path = "proxy_child/lifecycle.rs"]
mod lifecycle;
#[path = "proxy_child/model.rs"]
mod model;
#[path = "proxy_child/pipe.rs"]
mod pipe;
#[path = "proxy_child/spec.rs"]
mod spec;
#[path = "proxy_child/time.rs"]
mod time;
#[path = "proxy_child/worker.rs"]
mod worker;

pub(super) use lifecycle::ChildIo;
pub(super) use pipe::write_admitted_line;
pub(super) use spec::ChildLimits;

#[cfg(test)]
mod tests;
