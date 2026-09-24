use std::net::TcpStream;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

use search_provider_protocol::request::RequestCancellation;

use super::{Reply, Terminal};

pub(super) type Outcome = Result<Reply, String>;

pub(super) struct ExchangeOutput {
    pub(super) reply: Reply,
    // Complete diagnostic or shutdown frames awaiting parent-owned delivery.
    pub(super) deferred: Vec<u8>,
}

pub(super) struct Exchange {
    pub(super) command: String,
    pub(super) socket: TcpStream,
    pub(super) terminal: Terminal,
    pub(super) deadline: Instant,
    pub(super) cancellation: Option<RequestCancellation>,
    pub(super) reply: SyncSender<Result<ExchangeOutput, String>>,
}

impl Exchange {
    /// Only these exact admitted, single-frame diagnostic reads may finish
    /// draining after cancellation. Never infer safety from `Terminal::Single`
    /// alone: mutations and unknown commands must retain fail-stop behavior.
    pub(super) fn drains_cancellation(&self) -> bool {
        self.cancellation.is_some()
            && self.terminal == Terminal::Single
            && matches!(self.command.as_str(), "health" | "version")
    }
}
