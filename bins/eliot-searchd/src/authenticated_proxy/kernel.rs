//! Authenticated provider routing over one owner-fenced DIRECT child.

#[path = "../proxy_exchange.rs"]
mod exchange;
#[path = "../proxy_child.rs"]
mod child_io;

mod child;
mod dispatch;
mod entry;
mod envelope;
mod hello;
mod key;
mod operation;
mod server;
mod spec;
mod terminal;
mod wire;

use spec::MAX_PROXY_COMMAND_BYTES;
use terminal::Terminal;

pub use entry::maybe_run;
