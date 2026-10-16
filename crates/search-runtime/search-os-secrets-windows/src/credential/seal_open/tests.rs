//! Private key-window semantics over a recording original context.
//!
//! No native credential, DPAPI, RNG, caller-time API or clock issuer is used:
//! the fixture drives the private deadline directly and the original context
//! only records what the window forwards to it.

use super::{KeyWindow, RevisionLeaseRefusal};
use crate::{LegacyRevisionDpapiContext, LegacyRevisionDpapiWork, RevisionDekContext};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Cancelled,
    Exhausted,
}

/// Recording original context; the only ordinary-work meter the window may reach.
struct Recording {
    deadline: Instant,
    first: Option<Refusal>,
    checkpoints: usize,
    ordinary: Vec<LegacyRevisionDpapiWork>,
    opens: usize,
    native_clear_calls: usize,
    native_clear_bytes: usize,
}

impl Recording {
    fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            first: None,
            checkpoints: 0,
            ordinary: Vec::new(),
            opens: 0,
            native_clear_calls: 0,
            native_clear_bytes: 0,
        }
    }

    fn latch(&mut self, error: Refusal) -> Result<(), Refusal> {
        if self.first.is_none() {
            self.first = Some(error);
        }
        Err(self.first.unwrap_or(error))
    }
}

impl LegacyRevisionDpapiContext for Recording {
    type Error = Refusal;

    fn checkpoint(&mut self) -> Result<(), Refusal> {
        self.checkpoints += 1;
        if let Some(error) = self.first {
            return Err(error);
        }
        if Instant::now() >= self.deadline {
            return self.latch(Refusal::Cancelled);
        }
        Ok(())
    }

    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Refusal> {
        self.checkpoint()?;
        self.ordinary.push(work);
        Ok(())
    }

    fn account_native_clear(&mut self, actual_bytes: usize) -> Result<(), Refusal> {
        self.native_clear_calls += 1;
        self.native_clear_bytes = self.native_clear_bytes.saturating_add(actual_bytes);
        match self.first {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl RevisionDekContext for Recording {
    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn charge_open(&mut self) -> Result<(), Refusal> {
        self.checkpoint()?;
        self.opens += 1;
        Ok(())
    }
}

fn one_millisecond_ago() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_millis(1))
        .expect("monotonic clock is past process start")
}

fn past_deadline_window(original: &mut Recording) -> KeyWindow<'_, Recording> {
    KeyWindow {
        original,
        deadline: one_millisecond_ago(),
        expired: false,
    }
}

#[test]
fn expired_private_deadline_refuses_ordinary_work_before_the_original_is_touched() {
    let caller_deadline = Instant::now() + Duration::from_secs(30);
    let private_deadline = Instant::now() + Duration::from_secs(5);
    let mut original = Recording::new(caller_deadline);
    {
        let mut window = KeyWindow {
            original: &mut original,
            deadline: private_deadline,
            expired: false,
        };
        // The private limit is reported, not the original caller deadline.
        assert_eq!(window.deadline(), private_deadline);
        assert_ne!(window.deadline(), caller_deadline);
        assert!(window.checkpoint().is_ok());
        assert!(window.charge(LegacyRevisionDpapiWork::Allocate(64)).is_ok());
        assert!(window.charge_open().is_ok());
    }
    assert_eq!(original.opens, 1);
    assert_eq!(original.ordinary, [LegacyRevisionDpapiWork::Allocate(64)]);

    let mut original = Recording::new(Instant::now() + Duration::from_secs(30));
    {
        let mut window = past_deadline_window(&mut original);
        for _ in 0..2 {
            assert!(matches!(
                window.checkpoint(),
                Err(RevisionLeaseRefusal::Expired)
            ));
        }
        assert!(matches!(
            window.charge(LegacyRevisionDpapiWork::Process(8)),
            Err(RevisionLeaseRefusal::Expired)
        ));
        assert!(matches!(
            window.charge_open(),
            Err(RevisionLeaseRefusal::Expired)
        ));
    }
    // Expiry refuses before any underlying ordinary operation is reached.
    assert_eq!((original.checkpoints, original.opens), (0, 0));
    assert!(original.ordinary.is_empty());
}

#[test]
fn original_refusals_stay_typed_and_native_clear_still_reaches_the_original_meter() {
    // The original caller deadline already elapsed while the private limit is live.
    let mut original = Recording::new(one_millisecond_ago());
    {
        let mut window = KeyWindow {
            original: &mut original,
            deadline: Instant::now() + Duration::from_secs(5),
            expired: false,
        };
        assert!(matches!(
            window.checkpoint(),
            Err(RevisionLeaseRefusal::Original(Refusal::Cancelled))
        ));
        assert!(matches!(
            window.charge(LegacyRevisionDpapiWork::NativeCall),
            Err(RevisionLeaseRefusal::Original(Refusal::Cancelled))
        ));
        assert!(matches!(
            window.charge_open(),
            Err(RevisionLeaseRefusal::Original(Refusal::Cancelled))
        ));
    }
    // A live private deadline renews no ordinary allowance for the original.
    assert_eq!((original.opens, original.ordinary.len()), (0, 0));

    let mut original = Recording::new(Instant::now() + Duration::from_secs(30));
    {
        let mut window = past_deadline_window(&mut original);
        assert!(matches!(
            window.checkpoint(),
            Err(RevisionLeaseRefusal::Expired)
        ));
        // Mandatory cleanup bypasses the private expiry and debits the same meter.
        assert!(window.account_native_clear(4096).is_ok());
        assert!(matches!(
            window.checkpoint(),
            Err(RevisionLeaseRefusal::Expired)
        ));
        // The original's own refusal survives as Original, never as Expired.
        window.original.first = Some(Refusal::Exhausted);
        assert!(matches!(
            window.account_native_clear(7),
            Err(RevisionLeaseRefusal::Original(Refusal::Exhausted))
        ));
    }
    assert_eq!(
        (original.native_clear_calls, original.native_clear_bytes),
        (2, 4103)
    );
    assert_eq!((original.checkpoints, original.opens), (0, 0));
    assert!(original.ordinary.is_empty());
}

