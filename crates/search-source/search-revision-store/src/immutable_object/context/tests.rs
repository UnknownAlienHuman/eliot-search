//! Authored contextual owner cases; execution remains deferred to #215.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Context {
    deadline: Instant,
    cancelled: AtomicBool,
    polls: usize,
    refuse_poll: Option<usize>,
    bytes: usize,
    calls: usize,
    effect: bool,
    works: Vec<LegacyRevisionObjectWork>,
}
impl Context {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: AtomicBool::new(false),
            polls: 0,
            refuse_poll: None,
            bytes: 1024 * 1024,
            calls: 4096,
            effect: false,
            works: Vec::new(),
        }
    }
}
impl LegacyRevisionObjectContext for Context {
    type Error = &'static str;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.polls += 1;
        if self.refuse_poll == Some(self.polls) {
            self.cancelled.store(true, Ordering::Release);
        }
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            Err("PRIVATE_CONTEXT_CANARY")
        } else {
            Ok(())
        }
    }
    fn charge(&mut self, work: LegacyRevisionObjectWork) -> Result<(), Self::Error> {
        let bytes = match work {
            LegacyRevisionObjectWork::Read(n)
            | LegacyRevisionObjectWork::Write(n)
            | LegacyRevisionObjectWork::Allocate(n)
            | LegacyRevisionObjectWork::AllocateLocator(n)
            | LegacyRevisionObjectWork::Compare(n) => n,
            _ => 0,
        };
        if self.calls == 0 || bytes > self.bytes {
            self.cancelled.store(true, Ordering::Release);
            return Err("PRIVATE_BUDGET_CANARY");
        }
        self.calls -= 1;
        self.bytes -= bytes;
        self.works.push(work);
        Ok(())
    }
    fn effect_attempted(&mut self) {
        self.effect = true;
    }
}
struct TestPlatform {
    fail_sync: bool,
}

#[test]
fn cpu_progress_defaults_to_full_checks_and_never_replaces_io_guards() {
    let platform = TestPlatform { fail_sync: false };
    let mut default = Context::new();
    {
        let mut kernel = Kernel::new(&platform, &mut default, true);
        assert!(
            kernel
                .compare(b"same", b"same")
                .unwrap_or_else(|_| panic!("compare"))
        );
    }
    assert_eq!(
        default.polls, 3,
        "default keeps all original CPU checkpoints"
    );
    assert_eq!(default.works, [LegacyRevisionObjectWork::Compare(4)]);

    struct Progress {
        original: Context,
        full: usize,
        pure: usize,
    }
    impl LegacyRevisionObjectContext for Progress {
        type Error = &'static str;
        fn checkpoint(&mut self) -> Result<(), Self::Error> {
            self.full += 1;
            self.original.checkpoint()
        }
        fn progress(&mut self) -> Result<(), Self::Error> {
            self.pure += 1;
            self.original.checkpoint()
        }
        fn charge(&mut self, work: LegacyRevisionObjectWork) -> Result<(), Self::Error> {
            self.original.charge(work)
        }
        fn effect_attempted(&mut self) {
            self.original.effect_attempted();
        }
    }
    let mut context = Progress {
        original: Context::new(),
        full: 0,
        pure: 0,
    };
    {
        let mut kernel = Kernel::new(&platform, &mut context, true);
        assert!(
            kernel
                .compare(b"same", b"same")
                .unwrap_or_else(|_| panic!("compare"))
        );
        let result = kernel.read_data(&mut io::Cursor::new(b"data"), 4);
        let bytes = kernel
            .finish(result)
            .expect("read plus full terminal fence");
        assert_eq!(&*bytes, b"data");
    }
    assert_eq!((context.pure, context.full), (9, 7));
    assert_eq!(
        context.original.works,
        [
            LegacyRevisionObjectWork::Compare(4),
            LegacyRevisionObjectWork::Compare(5),
            LegacyRevisionObjectWork::Allocate(5),
            LegacyRevisionObjectWork::Read(5),
            LegacyRevisionObjectWork::Read(1)
        ]
    );
    assert_eq!(context.original.bytes, 1024 * 1024 - 20);
    assert!(!context.original.effect);
    context.original.cancelled.store(true, Ordering::Release);
    let mut kernel = Kernel::new(&platform, &mut context, true);
    assert!(matches!(
        kernel.compare(b"same", b"same"),
        Err(Failure::Context("PRIVATE_CONTEXT_CANARY"))
    ));
    drop(kernel);
    assert_eq!(
        context.original.works.len(),
        5,
        "refusal precedes CPU work admission"
    );
}
impl Platform for TestPlatform {
    type Identity = u64;
    type Error = io::Error;
    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            Ok(())
        } else {
            Err(io::Error::other("invalid directory"))
        }
    }
    fn verify_locator(&self, _: &File, path: &Path) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if regular(&metadata) {
            Ok(())
        } else {
            Err(io::Error::other("invalid file"))
        }
    }
    fn identity(&self, _: &File) -> Result<u64, Self::Error> {
        Ok(1)
    }
    fn sync_directory(&self, _: &Path) -> Result<(), Self::Error> {
        if self.fail_sync {
            Err(io::Error::other("PRIVATE_PLATFORM_CANARY"))
        } else {
            Ok(())
        }
    }
}
fn platform_attempt<C: LegacyRevisionObjectContext<Error = &'static str>, T>(
    ctx: &mut C,
    work: LegacyRevisionObjectWork,
    call: impl FnOnce() -> io::Result<T>,
) -> Result<T, LegacyRevisionObjectPlatformFailure<io::Error, &'static str>> {
    ctx.checkpoint()
        .map_err(LegacyRevisionObjectPlatformFailure::Context)?;
    ctx.charge(work)
        .map_err(LegacyRevisionObjectPlatformFailure::Context)?;
    let result = call();
    match (result, ctx.checkpoint()) {
        (Err(platform), Err(context)) => {
            Err(LegacyRevisionObjectPlatformFailure::AfterPlatform { platform, context })
        }
        (Err(platform), _) => Err(LegacyRevisionObjectPlatformFailure::Platform(platform)),
        (_, Err(context)) => Err(LegacyRevisionObjectPlatformFailure::Context(context)),
        (Ok(value), Ok(())) => Ok(value),
    }
}
impl<C: LegacyRevisionObjectContext<Error = &'static str>> ContextualLegacyRevisionObjectPlatform<C>
    for TestPlatform
{
    fn validate_directory_contextual(
        &self,
        ctx: &mut C,
        path: &Path,
    ) -> Result<(), LegacyRevisionObjectPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyRevisionObjectWork::Observe, || {
            self.validate_directory(path)
        })
    }
    fn verify_locator_contextual(
        &self,
        ctx: &mut C,
        file: &File,
        path: &Path,
    ) -> Result<(), LegacyRevisionObjectPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyRevisionObjectWork::Observe, || {
            self.verify_locator(file, path)
        })
    }
    fn identity_contextual(
        &self,
        ctx: &mut C,
        file: &File,
    ) -> Result<u64, LegacyRevisionObjectPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyRevisionObjectWork::Observe, || {
            self.identity(file)
        })
    }
    fn sync_directory_contextual(
        &self,
        ctx: &mut C,
        path: &Path,
    ) -> Result<(), LegacyRevisionObjectPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyRevisionObjectWork::Effect, || {
            self.sync_directory(path)
        })
    }
}
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "eliot-contextual-revision-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("fixture");
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn contextual_read_declines_at_every_original_checkpoint() {
    let fixture = Fixture::new();
    let path = fixture.0.join("object");
    fs::write(&path, b"PRIVATE_BYTES_CANARY").expect("object");
    let platform = TestPlatform { fail_sync: false };
    let mut baseline = Context::new();
    let read = read_legacy_revision_object_with_context(&platform, &mut baseline, &path, 64)
        .expect("read");
    assert_eq!(read.bytes(), b"PRIVATE_BYTES_CANARY");
    assert!(!format!("{read:?}").contains("PRIVATE_BYTES_CANARY"));
    for poll in 1..=baseline.polls {
        let mut context = Context::new();
        let deadline = context.deadline;
        context.refuse_poll = Some(poll);
        let error = read_legacy_revision_object_with_context(&platform, &mut context, &path, 64)
            .expect_err("refusal");
        assert!(!error.outcome_unknown());
        assert!(!context.effect);
        assert_eq!(context.deadline, deadline);
        assert!(!format!("{error:?} {error}").contains("PRIVATE_CONTEXT_CANARY"));
        assert_eq!(fs::read(&path).expect("retained"), b"PRIVATE_BYTES_CANARY");
    }
}

#[test]
fn contextual_preflight_refuses_without_dispatching_effects() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("aa");
    let platform = TestPlatform { fail_sync: false };
    for state in 0..4 {
        let mut context = Context::new();
        match state {
            0 => context.cancelled.store(true, Ordering::Release),
            1 => context.deadline = Instant::now(),
            2 => context.calls = 0,
            _ => {}
        }
        let name = if state == 3 { "../escape" } else { "object" };
        let error = publish_legacy_revision_object_with_context(
            &platform,
            &mut context,
            &shard,
            name,
            ".object.tmp",
            b"x",
            8,
        )
        .expect_err("preflight refusal");
        assert!(!error.outcome_unknown());
        assert!(!context.effect);
        assert!(!shard.exists());
    }
}

#[test]
fn contextual_success_retains_stage_and_legacy_cleanup_remains_distinct() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("bb");
    let platform = TestPlatform { fail_sync: false };
    let mut context = Context::new();
    let receipt = publish_legacy_revision_object_with_context(
        &platform,
        &mut context,
        &shard,
        "object",
        ".object.tmp",
        b"bytes",
        8,
    )
    .expect("publication");
    assert!(receipt.staging_cleanup_deferred());
    assert!(!receipt.reused());
    assert_eq!(fs::read(shard.join("object")).expect("final"), b"bytes");
    assert_eq!(
        fs::read(shard.join(".object.tmp")).expect("stage"),
        b"bytes"
    );
    assert_eq!(receipt.encoded_bytes(), 5);
    let legacy = super::super::publish_legacy_revision_object(
        &platform,
        &shard,
        "legacy",
        ".legacy.tmp",
        b"legacy",
        8,
    )
    .expect("legacy");
    assert!(!legacy.reused());
    assert!(!shard.join(".legacy.tmp").exists());
}

#[test]
fn refusal_after_real_creation_or_publication_preserves_artifacts() {
    let fixture = Fixture::new();
    let platform = TestPlatform { fail_sync: false };
    let baseline_shard = fixture.0.join("baseline");
    let mut baseline = Context::new();
    let receipt = publish_legacy_revision_object_with_context(
        &platform,
        &mut baseline,
        &baseline_shard,
        "object",
        ".object.tmp",
        b"bytes",
        8,
    )
    .expect("baseline");
    assert!(receipt.staging_cleanup_deferred());
    let mut witnessed_stage = false;
    let mut witnessed_final = false;
    for poll in 1..=baseline.polls {
        let shard = fixture.0.join(format!("fault-{poll}"));
        let mut context = Context::new();
        context.refuse_poll = Some(poll);
        let error = publish_legacy_revision_object_with_context(
            &platform,
            &mut context,
            &shard,
            "object",
            ".object.tmp",
            b"bytes",
            8,
        )
        .expect_err("refusal");
        assert_eq!(error.outcome_unknown(), context.effect);
        if shard.join(".object.tmp").exists() {
            witnessed_stage = true;
            assert!(error.outcome_unknown());
        }
        if shard.join("object").exists() {
            witnessed_final = true;
            assert!(error.outcome_unknown());
            assert_eq!(
                error.effect(),
                ContextualRevisionObjectEffect::PublicationAttempted
            );
            assert_eq!(
                fs::read(shard.join("object")).expect("retained final"),
                b"bytes"
            );
            assert!(shard.join(".object.tmp").exists());
        }
    }
    assert!(witnessed_stage && witnessed_final);
    let shard = fixture.0.join("sync-failure");
    fs::create_dir(&shard).expect("shard");
    let mut context = Context::new();
    let error = publish_legacy_revision_object_with_context(
        &TestPlatform { fail_sync: true },
        &mut context,
        &shard,
        "object",
        ".object.tmp",
        b"bytes",
        8,
    )
    .expect_err("sync failure");
    assert!(error.outcome_unknown());
    assert!(shard.join("object").exists());
    assert!(shard.join(".object.tmp").exists());
    assert!(!format!("{error:?} {error}").contains("PRIVATE_PLATFORM_CANARY"));
    // A nested platform failure and its post-call caller refusal both survive.
    let mut context = Context::new();
    context.refuse_poll = Some(4);
    let platform = TestPlatform { fail_sync: true };
    let mut kernel = Kernel::new(&platform, &mut context, true);
    let result = kernel.platform_call(
        ContextualRevisionObjectEffect::PublicationAttempted,
        ObjectError::PublishPlatformOutcomeUnknown,
        |platform, context| platform.sync_directory_contextual(context, &shard),
    );
    let error = kernel.finish(result).expect_err("paired failure");
    assert_eq!(
        error.object_code(),
        Some("REVISION_OBJECT_DURABILITY_FAILED")
    );
    assert_eq!(error.context_cause(), Some(&"PRIVATE_CONTEXT_CANARY"));
    assert!(error.outcome_unknown());
    assert!(!format!("{error:?} {error}").contains("PRIVATE_"));
}

struct ShortRead {
    bytes: io::Cursor<Vec<u8>>,
    interrupted: bool,
    calls: usize,
}
impl Read for ShortRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = buffer.len().min(1);
        self.bytes.read(&mut buffer[..count])
    }
}
struct ShortWrite {
    bytes: Vec<u8>,
    interrupted: bool,
}
impl Write for ShortWrite {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.bytes.push(buffer[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn short_io_consumes_requested_bytes_across_the_same_allowance() {
    let platform = TestPlatform { fail_sync: false };
    // Both the guarded buffer and its mandatory wipe are admitted before reads.
    let exact = (4 + 4 + 3 + 2 + 1) + 2 * 5 + (5 + 5 + 4 + 3 + 2 + 1);
    assert_eq!(exact, 44);
    for ceiling in [exact, exact - 1] {
        let mut context = Context::new();
        context.bytes = ceiling;
        let mut writer = ShortWrite {
            bytes: Vec::new(),
            interrupted: false,
        };
        let mut reader = ShortRead {
            bytes: io::Cursor::new(b"1234".to_vec()),
            interrupted: false,
            calls: 0,
        };
        let mut kernel = Kernel::new(&platform, &mut context, true);
        kernel
            .write_data(&mut writer, b"1234")
            .unwrap_or_else(|_| panic!("write"));
        let result = kernel.read_data(&mut reader, 4);
        let result = kernel.finish(result);
        drop(kernel);
        assert_eq!(writer.bytes, b"1234");
        let writes = context
            .works
            .iter()
            .filter_map(|work| {
                if let LegacyRevisionObjectWork::Write(n) = work {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(writes, [4, 4, 3, 2, 1]);
        if ceiling == exact {
            assert_eq!(&*result.expect("exact allowance"), b"1234");
            assert_eq!(context.bytes, 0);
            assert_eq!(reader.calls, 6);
        } else {
            assert!(result.expect_err("EOF probe refused").outcome_unknown());
            assert_eq!(reader.calls, 5);
        }
    }
}

#[test]
fn existing_exact_reuse_and_conflict_dispatch_no_mutation() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("cc");
    fs::create_dir(&shard).expect("shard");
    let platform = TestPlatform { fail_sync: false };
    for (name, bytes) in [("empty", b"".as_slice()), ("nonempty", b"bytes".as_slice())] {
        fs::write(shard.join(name), bytes).expect("existing");
        let mut context = Context::new();
        let receipt = publish_legacy_revision_object_with_context(
            &platform,
            &mut context,
            &shard,
            name,
            ".unused.tmp",
            bytes,
            8,
        )
        .expect("reuse");
        assert!(receipt.reused());
        assert!(!receipt.staging_cleanup_deferred());
        assert!(!context.effect);
        let mut context = Context::new();
        let error = publish_legacy_revision_object_with_context(
            &platform,
            &mut context,
            &shard,
            name,
            ".unused.tmp",
            b"unequal",
            8,
        )
        .expect_err("conflict");
        assert!(!error.outcome_unknown());
        assert!(!context.effect);
        assert_eq!(fs::read(shard.join(name)).expect("preserved"), bytes);
        assert!(!shard.join(".unused.tmp").exists());
    }
}
