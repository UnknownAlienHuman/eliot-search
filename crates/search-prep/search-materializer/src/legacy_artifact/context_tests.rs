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
    works: Vec<LegacyPreparationArtifactWork>,
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
impl LegacyPreparationArtifactContext for Context {
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
    fn charge(&mut self, work: LegacyPreparationArtifactWork) -> Result<(), Self::Error> {
        let bytes = match work {
            LegacyPreparationArtifactWork::Read(n)
            | LegacyPreparationArtifactWork::Write(n)
            | LegacyPreparationArtifactWork::Allocate(n)
            | LegacyPreparationArtifactWork::AllocateLocator(n)
            | LegacyPreparationArtifactWork::Compare(n) => n,
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
fn platform_attempt<T>(
    ctx: &mut Context,
    work: LegacyPreparationArtifactWork,
    call: impl FnOnce() -> io::Result<T>,
) -> Result<T, LegacyPreparationArtifactPlatformFailure<io::Error, &'static str>> {
    ctx.checkpoint()
        .map_err(LegacyPreparationArtifactPlatformFailure::Context)?;
    ctx.charge(work)
        .map_err(LegacyPreparationArtifactPlatformFailure::Context)?;
    let result = call();
    match (result, ctx.checkpoint()) {
        (Err(platform), Err(context)) => {
            Err(LegacyPreparationArtifactPlatformFailure::AfterPlatform { platform, context })
        }
        (Err(platform), _) => Err(LegacyPreparationArtifactPlatformFailure::Platform(platform)),
        (_, Err(context)) => Err(LegacyPreparationArtifactPlatformFailure::Context(context)),
        (Ok(value), Ok(())) => Ok(value),
    }
}
impl ContextualLegacyPreparationArtifactPlatform<Context> for TestPlatform {
    fn validate_directory_contextual(
        &self,
        ctx: &mut Context,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyPreparationArtifactWork::Observe, || {
            self.validate_directory(path)
        })
    }
    fn verify_locator_contextual(
        &self,
        ctx: &mut Context,
        file: &File,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyPreparationArtifactWork::Observe, || {
            self.verify_locator(file, path)
        })
    }
    fn identity_contextual(
        &self,
        ctx: &mut Context,
        file: &File,
    ) -> Result<u64, LegacyPreparationArtifactPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyPreparationArtifactWork::Observe, || {
            self.identity(file)
        })
    }
    fn sync_directory_contextual(
        &self,
        ctx: &mut Context,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<io::Error, &'static str>> {
        platform_attempt(ctx, LegacyPreparationArtifactWork::Effect, || {
            self.sync_directory(path)
        })
    }
}
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "eliot-contextual-preparation-{}-{}",
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
    let path = fixture.0.join("artifact.prep");
    fs::write(&path, b"PRIVATE_BYTES_CANARY").expect("artifact.prep");
    let platform = TestPlatform { fail_sync: false };
    let mut baseline = Context::new();
    let read = read_legacy_preparation_artifact_with_context(&platform, &mut baseline, &path, 64)
        .expect("read");
    assert_eq!(read.bytes(), b"PRIVATE_BYTES_CANARY");
    assert!(!format!("{read:?}").contains("PRIVATE_BYTES_CANARY"));
    for poll in 1..=baseline.polls {
        let mut context = Context::new();
        let deadline = context.deadline;
        context.refuse_poll = Some(poll);
        let error =
            read_legacy_preparation_artifact_with_context(&platform, &mut context, &path, 64)
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
        let name = if state == 3 {
            "../escape"
        } else {
            "artifact.prep"
        };
        let error = publish_legacy_preparation_artifact_with_context(
            &platform,
            &mut context,
            &shard,
            name,
            ".artifact.prep.tmp",
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
    let receipt = publish_legacy_preparation_artifact_with_context(
        &platform,
        &mut context,
        &shard,
        "artifact.prep",
        ".artifact.prep.tmp",
        b"bytes",
        8,
    )
    .expect("publication");
    assert!(receipt.staging_cleanup_deferred());
    assert!(!receipt.reused());
    assert_eq!(
        fs::read(shard.join("artifact.prep")).expect("final"),
        b"bytes"
    );
    assert_eq!(
        fs::read(shard.join(".artifact.prep.tmp")).expect("stage"),
        b"bytes"
    );
    assert_eq!(receipt.encoded_bytes(), 5);
    let legacy = super::super::publish_legacy_preparation_artifact(
        &platform,
        &shard,
        "legacy.ref",
        ".legacy.ref.tmp",
        b"legacy",
        8,
    )
    .expect("legacy.ref");
    assert!(!legacy.reused());
    assert!(!shard.join(".legacy.ref.tmp").exists());
}

#[test]
fn refusal_after_real_creation_or_publication_preserves_artifacts() {
    let fixture = Fixture::new();
    let platform = TestPlatform { fail_sync: false };
    let baseline_shard = fixture.0.join("baseline");
    let mut baseline = Context::new();
    publish_legacy_preparation_artifact_with_context(
        &platform,
        &mut baseline,
        &baseline_shard,
        "artifact.prep",
        ".artifact.prep.tmp",
        b"bytes",
        8,
    )
    .expect("baseline");
    let mut witnessed_stage = false;
    let mut witnessed_final = false;
    for poll in 1..=baseline.polls {
        let shard = fixture.0.join(format!("fault-{poll}"));
        let mut context = Context::new();
        context.refuse_poll = Some(poll);
        let error = publish_legacy_preparation_artifact_with_context(
            &platform,
            &mut context,
            &shard,
            "artifact.prep",
            ".artifact.prep.tmp",
            b"bytes",
            8,
        )
        .expect_err("refusal");
        assert_eq!(error.outcome_unknown(), context.effect);
        if shard.join(".artifact.prep.tmp").exists() {
            witnessed_stage = true;
            assert!(error.outcome_unknown());
        }
        if shard.join("artifact.prep").exists() {
            witnessed_final = true;
            assert!(error.outcome_unknown());
            assert_eq!(
                error.effect(),
                ContextualPreparationArtifactEffect::PublicationAttempted
            );
            assert_eq!(
                fs::read(shard.join("artifact.prep")).expect("retained final"),
                b"bytes"
            );
            assert!(shard.join(".artifact.prep.tmp").exists());
        }
    }
    assert!(witnessed_stage && witnessed_final);
    let shard = fixture.0.join("sync-failure");
    fs::create_dir(&shard).expect("shard");
    let mut context = Context::new();
    let error = publish_legacy_preparation_artifact_with_context(
        &TestPlatform { fail_sync: true },
        &mut context,
        &shard,
        "artifact.prep",
        ".artifact.prep.tmp",
        b"bytes",
        8,
    )
    .expect_err("sync failure");
    assert!(error.outcome_unknown());
    assert!(shard.join("artifact.prep").exists());
    assert!(shard.join(".artifact.prep.tmp").exists());
    assert!(!format!("{error:?} {error}").contains("PRIVATE_PLATFORM_CANARY"));
    // A nested platform failure and its post-call caller refusal both survive.
    let mut context = Context::new();
    context.refuse_poll = Some(4);
    let platform = TestPlatform { fail_sync: true };
    let mut kernel = Kernel::new(&platform, &mut context, true);
    let result = kernel.platform_call(
        ContextualPreparationArtifactEffect::PublicationAttempted,
        ObjectError::PublishPlatformOutcomeUnknown,
        |platform, context| platform.sync_directory_contextual(context, &shard),
    );
    let error = kernel.finish(result).expect_err("paired failure");
    assert_eq!(
        error.object_code(),
        Some("PREPARATION_ARTIFACT_DURABILITY_FAILED")
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
struct ZeroWrite {
    calls: usize,
}
impl Write for ZeroWrite {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        Ok(0)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn short_io_consumes_requested_bytes_across_the_same_allowance() {
    let platform = TestPlatform { fail_sync: false };
    for ceiling in [39, 38] {
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
                if let LegacyPreparationArtifactWork::Write(n) = work {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(writes, [4, 4, 3, 2, 1]);
        if ceiling == 39 {
            assert_eq!(result.expect("exact allowance"), b"1234");
            assert_eq!(context.bytes, 0);
            assert_eq!(reader.calls, 6);
        } else {
            assert!(result.expect_err("EOF probe refused").outcome_unknown());
            assert_eq!(reader.calls, 5);
        }
    }
    let mut legacy_context = LegacyContext;
    let legacy_platform = LegacyPlatform(&platform);
    let mut legacy = Kernel::new(&legacy_platform, &mut legacy_context, false);
    let mut zero = ZeroWrite { calls: 0 };
    let error =
        legacy_result(legacy.write_data(&mut zero, b"1234")).expect_err("legacy zero progress");
    assert!(
        matches!(&error, ObjectError::Write(cause) if cause.kind() == io::ErrorKind::WriteZero)
    );
    assert_eq!(
        error.to_string(),
        "preparation artifact write failed: failed to write whole buffer"
    );
    assert_eq!(zero.calls, 1);

    let mut context = Context::new();
    context.bytes = 4;
    let mut kernel = Kernel::new(&platform, &mut context, true);
    let mut zero = ZeroWrite { calls: 0 };
    let result = kernel.write_data(&mut zero, b"1234");
    let error = kernel.finish(result).expect_err("contextual zero progress");
    drop(kernel);
    assert!(
        matches!(&error.failure, Failure::Object(ObjectError::Write(cause)) if cause.kind() == io::ErrorKind::WriteZero)
    );
    assert_eq!(
        error.object_code(),
        Some("PREPARATION_ARTIFACT_WRITE_FAILED")
    );
    assert_eq!(
        error.effect(),
        ContextualPreparationArtifactEffect::StagingAttempted
    );
    assert_eq!(error.code(), "PREPARATION_ARTIFACT_OUTCOME_UNKNOWN");
    assert!(error.outcome_unknown());
    assert_eq!(error.context_cause(), None);
    assert_eq!(context.bytes, 0);
    assert_eq!(context.works, [LegacyPreparationArtifactWork::Write(4)]);
    assert_eq!(zero.calls, 1);
}

#[test]
fn existing_exact_reuse_and_conflict_dispatch_no_mutation() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("cc");
    fs::create_dir(&shard).expect("shard");
    let platform = TestPlatform { fail_sync: false };
    for (name, bytes) in [
        ("empty.ref", b"".as_slice()),
        ("nonempty.prep", b"bytes".as_slice()),
    ] {
        fs::write(shard.join(name), bytes).expect("existing");
        let mut context = Context::new();
        let receipt = publish_legacy_preparation_artifact_with_context(
            &platform,
            &mut context,
            &shard,
            name,
            ".unused.ref.tmp",
            bytes,
            8,
        )
        .expect("reuse");
        assert!(receipt.reused());
        assert!(!receipt.staging_cleanup_deferred());
        assert!(!context.effect);
        let mut context = Context::new();
        let error = publish_legacy_preparation_artifact_with_context(
            &platform,
            &mut context,
            &shard,
            name,
            ".unused.ref.tmp",
            b"unequal",
            8,
        )
        .expect_err("conflict");
        assert!(!error.outcome_unknown());
        assert!(!context.effect);
        assert_eq!(fs::read(shard.join(name)).expect("preserved"), bytes);
        assert!(!shard.join(".unused.ref.tmp").exists());
    }
}
