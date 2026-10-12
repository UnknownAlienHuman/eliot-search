//! Authored owning context cases; no native/runtime execution is asserted.
use super::*;
use crate::{LegacyRevisionDpapiError, MAX_LEGACY_REVISION_DPAPI_BYTES};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Cancelled,
    Expired,
    Exhausted,
}
struct Original<'a> {
    flag: &'a AtomicBool,
    deadline: Instant,
    capacity: usize,
    processing: usize,
    calls: usize,
    cap: usize,
    processing_cap: usize,
    call_cap: usize,
    first: Option<Refusal>,
    events: Vec<LegacyRevisionDpapiWork>,
    native_clear_attempts: Vec<usize>,
    cleanup_overflow: bool,
}
impl<'a> Original<'a> {
    fn new(flag: &'a AtomicBool, deadline: Instant, cap: usize) -> Self {
        Self {
            flag,
            deadline,
            capacity: 0,
            processing: 0,
            calls: 0,
            cap,
            processing_cap: usize::MAX,
            call_cap: usize::MAX,
            first: None,
            events: Vec::new(),
            native_clear_attempts: Vec::new(),
            cleanup_overflow: false,
        }
    }
    fn latch(&mut self, error: Refusal) -> Result<(), Refusal> {
        if self.first.is_none() {
            self.first = Some(error);
        }
        Err(self.first.unwrap())
    }
}
impl LegacyRevisionDpapiContext for Original<'_> {
    type Error = Refusal;
    fn checkpoint(&mut self) -> Result<(), Refusal> {
        if let Some(error) = self.first {
            return Err(error);
        }
        if self.flag.load(Ordering::SeqCst) {
            return self.latch(Refusal::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return self.latch(Refusal::Expired);
        }
        Ok(())
    }
    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Refusal> {
        self.checkpoint()?;
        self.events.push(work);
        let (total, n, maximum) = match work {
            LegacyRevisionDpapiWork::Allocate(n) => (&mut self.capacity, n, self.cap),
            LegacyRevisionDpapiWork::Process(n) => (&mut self.processing, n, self.processing_cap),
            LegacyRevisionDpapiWork::NativeCall => (&mut self.calls, 1, self.call_cap),
        };
        match total.checked_add(n).filter(|v| *v <= maximum) {
            Some(next) => {
                *total = next;
                Ok(())
            }
            None => self.latch(Refusal::Exhausted),
        }
    }
    fn account_native_clear(&mut self, n: usize) -> Result<(), Refusal> {
        self.native_clear_attempts.push(n);
        let attempted = self.processing.checked_add(n);
        self.cleanup_overflow |= attempted.is_none();
        self.processing = attempted.unwrap_or(usize::MAX);
        let polled = self.checkpoint();
        if self.cleanup_overflow || self.processing > self.processing_cap {
            self.latch(Refusal::Exhausted)
        } else {
            polled
        }
    }
}
fn live<'a>(flag: &'a AtomicBool, cap: usize) -> Original<'a> {
    Original::new(flag, Instant::now() + Duration::from_secs(10), cap)
}

#[test]
fn exact_output_capacity_is_charged_before_copy_not_structural_maximum() {
    let flag = AtomicBool::new(false);
    let mut c = live(&flag, 7);
    reserve::<_, ()>(&mut c, false).unwrap();
    assert_eq!(c.capacity, 0);
    assert_eq!(c.processing, 0);
    assert_eq!(c.calls, 3);
    let output = copy::<_, ()>(&mut c, b"1234567", true).unwrap().take();
    assert_eq!(output, b"1234567");
    assert_eq!(c.capacity, 7);
    assert!(matches!(
        c.events.last(),
        Some(LegacyRevisionDpapiWork::Process(14))
    ));
    let mut too_small = live(&flag, 6);
    assert!(matches!(
        copy::<_, ()>(&mut too_small, b"1234567", true),
        Err(KernelError::Context(Refusal::Exhausted))
    ));
    assert_eq!(too_small.processing, 0);
}

#[test]
fn original_cancellation_expiry_and_exhaustion_remain_sticky() {
    let flag = AtomicBool::new(true);
    let mut c = live(&flag, 10);
    assert!(matches!(
        copy::<_, ()>(&mut c, b"x", true),
        Err(KernelError::Context(Refusal::Cancelled))
    ));
    assert!(c.events.is_empty());
    flag.store(false, Ordering::SeqCst);
    assert_eq!(c.checkpoint(), Err(Refusal::Cancelled));
    let mut expired = Original::new(&flag, Instant::now(), 10);
    assert!(matches!(
        copy::<_, ()>(&mut expired, b"x", true),
        Err(KernelError::Context(Refusal::Expired))
    ));
    assert!(expired.events.is_empty());
    let mut exhausted = live(&flag, 0);
    assert!(matches!(
        copy::<_, ()>(&mut exhausted, b"x", true),
        Err(KernelError::Context(Refusal::Exhausted))
    ));
    exhausted.cap = 10;
    assert_eq!(exhausted.checkpoint(), Err(Refusal::Exhausted));
}

#[test]
fn native_failure_and_original_post_refusal_keep_both_redacted_causes() {
    let flag = AtomicBool::new(false);
    let mut c = live(&flag, 32);
    let result = stage(&mut c, || {
        flag.store(true, Ordering::SeqCst);
        Err::<(), _>(LegacyRevisionDpapiError::ProtectFailed(998))
    });
    let error = ContextualLegacyRevisionDpapiError::from(result.unwrap_err());
    assert!(matches!(
        error,
        ContextualLegacyRevisionDpapiError::NativeAndContext {
            native: LegacyRevisionDpapiError::ProtectFailed(998),
            context: Refusal::Cancelled,
        }
    ));
    assert_eq!(
        format!("{error:?}"),
        "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT"
    );
    assert_eq!(
        format!("{error}"),
        "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT"
    );
}

#[test]
fn cleanup_attempts_are_reserved_and_actual_clear_survives_refusal() {
    let flag = AtomicBool::new(false);
    let mut c = live(&flag, 0);
    reserve::<_, ()>(&mut c, true).unwrap();
    assert_eq!(
        c.events,
        vec![
            LegacyRevisionDpapiWork::NativeCall,
            LegacyRevisionDpapiWork::NativeCall,
            LegacyRevisionDpapiWork::NativeCall,
            LegacyRevisionDpapiWork::NativeCall
        ]
    );
    let mut short = live(&flag, 0);
    short.call_cap = 3;
    assert!(matches!(
        reserve::<_, ()>(&mut short, true),
        Err(KernelError::Context(Refusal::Exhausted))
    ));
    // No ABI dispatch follows a failed call reservation.
    let mut clears = 0;
    let result = finish_native_clear::<_, (), _>(&mut c, 7, Ok(()), || clears += 1);
    assert!(result.is_ok());
    assert_eq!((c.processing, clears), (7, 1));
    assert_eq!(c.native_clear_attempts, [7]);
    flag.store(true, Ordering::SeqCst);
    assert_eq!(c.checkpoint(), Err(Refusal::Cancelled));
    let result = finish_native_clear::<_, (), _>(&mut c, 3, Ok(()), || clears += 1);
    assert!(matches!(
        result,
        Err(KernelError::Context(Refusal::Cancelled))
    ));
    assert_eq!((c.processing, clears), (10, 2));
    assert_eq!(c.native_clear_attempts, [7, 3]);
    flag.store(false, Ordering::SeqCst);
    assert_eq!(c.checkpoint(), Err(Refusal::Cancelled));
}

#[test]
fn cleanup_ceiling_overflow_and_native_failure_refuse_success_without_skipping_clear() {
    let flag = AtomicBool::new(false);
    let mut c = live(&flag, 0);
    c.processing_cap = 3;
    let mut clears = 0;
    let result = finish_native_clear::<_, (), _>(&mut c, 4, Ok(()), || clears += 1);
    assert!(matches!(
        result,
        Err(KernelError::Context(Refusal::Exhausted))
    ));
    assert_eq!((c.processing, clears), (4, 1));
    assert_eq!(c.checkpoint(), Err(Refusal::Exhausted));
    let result = finish_native_clear(
        &mut c,
        2,
        Err::<(), _>(KernelError::Object(
            LegacyRevisionDpapiError::ProtectFailed(5),
        )),
        || clears += 1,
    );
    assert!(matches!(
        result,
        Err(KernelError::AfterObject {
            object: LegacyRevisionDpapiError::ProtectFailed(5),
            context: Refusal::Exhausted
        })
    ));
    assert_eq!((c.processing, clears), (6, 2));
    c.processing = usize::MAX;
    let result = finish_native_clear::<_, (), _>(&mut c, 1, Ok(()), || clears += 1);
    assert!(matches!(
        result,
        Err(KernelError::Context(Refusal::Exhausted))
    ));
    assert!(c.cleanup_overflow);
    assert_eq!((c.processing, clears), (usize::MAX, 3));
}

#[test]
fn legacy_signatures_bytes_and_error_variants_are_preserved() {
    type Old = fn(&mut [u8], &[u8; 32]) -> Result<Vec<u8>, LegacyRevisionDpapiError>;
    let _: Old = crate::protect_legacy_revision_current_user;
    let _: Old = crate::unprotect_legacy_revision_current_user;
    let output = legacy(copy::<_, LegacyRevisionDpapiError>(
        &mut LegacyContext,
        b"raw\0bytes\r\n",
        false,
    ))
    .unwrap()
    .take();
    assert_eq!(output, b"raw\0bytes\r\n");
    assert_eq!(
        legacy::<_, ()>(Err(KernelError::Object(
            LegacyRevisionDpapiError::ProtectFailed(5)
        ))),
        Err(LegacyRevisionDpapiError::ProtectFailed(5))
    );
    assert_eq!(MAX_LEGACY_REVISION_DPAPI_BYTES, 65 * 1024 * 1024);
}

#[cfg(windows)]
#[test]
fn actual_contextual_native_round_trip_uses_reported_output_charges() {
    let flag = AtomicBool::new(false);
    let mut c = live(&flag, 1024 * 1024);
    let mut input = b"owning native context fixture".to_vec();
    let entropy = [7; 32];
    let mut protected =
        crate::protect_legacy_revision_current_user_with_context(&mut c, &mut input, &entropy)
            .unwrap();
    let cipher = protected.len();
    assert_eq!(c.capacity, 32 + cipher);
    let observed = crate::unprotect_legacy_revision_current_user_with_context(
        &mut c,
        &mut protected,
        &entropy,
    )
    .unwrap();
    assert_eq!(observed, input);
    assert_eq!(c.capacity, 64 + cipher + observed.len());
    assert_eq!(c.calls, 7);
    assert_eq!(c.native_clear_attempts, [cipher, observed.len()]);
    assert_eq!(c.processing, 128 + 3 * (cipher + observed.len()));
    assert_eq!(flag.load(Ordering::SeqCst), false);
}
