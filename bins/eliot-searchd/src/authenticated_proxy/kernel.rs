//! Authenticated provider routing over one owner-fenced DIRECT child.

#[path = "../proxy_exchange.rs"]
mod exchange;
#[path = "../proxy_child.rs"]
mod child_io;

#[path = "kernel/child.rs"]
mod child;
#[path = "kernel/dispatch.rs"]
mod dispatch;
#[path = "kernel/entry.rs"]
mod entry;
#[path = "kernel/envelope.rs"]
mod envelope;
#[path = "kernel/hello.rs"]
mod hello;
#[path = "kernel/key.rs"]
mod key;
#[path = "kernel/operation.rs"]
mod operation;
#[path = "kernel/server.rs"]
mod server;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/terminal.rs"]
mod terminal;
#[path = "kernel/wire.rs"]
mod wire;

use spec::MAX_PROXY_COMMAND_BYTES;
use terminal::Terminal;

pub use entry::maybe_run;
