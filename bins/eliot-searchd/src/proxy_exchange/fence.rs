use super::reply::Reply;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in super::super) struct ExchangeFence {
    blocked: bool,
}

impl ExchangeFence {
    pub(in super::super) const fn blocked(self) -> bool {
        self.blocked
    }

    /// Arms before writing any command byte. Every early return stays blocked.
    /// Only a fully consumed reusable response can release the fence; no write
    /// is retried.
    pub(in super::super) fn run(
        &mut self,
        exchange: impl FnOnce() -> Result<Reply, String>,
    ) -> Result<Reply, String> {
        if self.blocked {
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        self.blocked = true;
        let reply = exchange()?;
        if matches!(reply, Reply::Complete | Reply::Rejected) {
            self.blocked = false;
        }
        Ok(reply)
    }
}
