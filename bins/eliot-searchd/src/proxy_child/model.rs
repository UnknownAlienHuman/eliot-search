use std::net::TcpStream;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

use search_provider_protocol::request::RequestCancellation;

use super::{Reply, Terminal};

pub(super) type Outcome = Result<Reply, String>;

pub(super) struct ExchangeOutput {
    pub(super) reply: Reply,
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
