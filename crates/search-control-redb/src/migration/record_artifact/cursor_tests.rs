//! Real file/handle and offset proofs. TestPlatform's injected identity and
//! directory durability are component seams, not NativeCtx qualification.

use super::*;
use std::cell::Cell;

#[derive(Clone)]
struct OpeningPlatform {
    inner: TestPlatform,
    locator_opens: Arc<AtomicU64>,
}

impl SourceImportOutputLockPlatform for OpeningPlatform {
    type Error = io::Error;

    fn validate_directory(&self, path: &Path) -> io::Result<()> {
        self.inner.validate_directory(path)
    }

    fn verify_locator(&self, file: &File, path: &Path) -> io::Result<()> {
        // Keep a real fresh locator open at each existing verification seam.
        let current = File::open(path)?;
        self.locator_opens.fetch_add(1, Ordering::SeqCst);
        assert_ne!(handle_key(file), handle_key(&current));
        self.inner.verify_locator(file, path)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        self.inner.sync_directory(path)
    }
}

impl SourceImportOutputArtifactPlatform for OpeningPlatform {
    type Identity = u64;

    fn identity(&self, file: &File) -> io::Result<Self::Identity> {
        self.inner.identity(file)
    }

    fn record_locator_with_context(
        &self,
        file: &File,
        path: &Path,
        deadline: Instant,
        work: &dyn Fn(Work) -> Result<(), ControlError>,
    ) -> Result<(), SourceImportRecordArtifactError<io::Error>> {
        call(deadline, work, Work::Open, || {
            self.verify_locator(file, path)
                .map_err(SourceImportRecordArtifactError::Platform)
        })
    }
}

impl SourceImportRecordArtifactPlatform for OpeningPlatform {
    fn sync_record_artifact_directory(&self, path: &Path) -> io::Result<()> {
        self.inner.sync_record_artifact_directory(path)
    }
}

fn prepared_rows<P: SourceImportRecordArtifactPlatform>(
    fixture: &Fixture,
    platform: P,
    expected: &[Vec<u8>],
) -> SourceImportRecordArtifactPublication<P> {
    let mut artifact = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.cursor.tmp",
        platform,
        deadline(),
    )
    .unwrap_or_else(|_| panic!("create"));
    for row in expected {
        artifact
            .push(row, deadline())
            .unwrap_or_else(|_| panic!("row"));
    }
    let mut readback = artifact
        .freeze(deadline())
        .unwrap_or_else(|_| panic!("freeze"))
        .begin_readback(deadline())
        .unwrap_or_else(|_| panic!("readback"));
    for row in expected {
        readback
            .compare(row, deadline())
            .unwrap_or_else(|_| panic!("compare"));
    }
    readback
        .finish(deadline())
        .unwrap_or_else(|_| panic!("finish"))
        .prepare_publication("artifact.source-map.v1", deadline())
        .unwrap_or_else(|_| panic!("publication"))
}

fn multiple_buffers() -> Vec<Vec<u8>> {
    (b'a'..=b'd')
        .map(|byte| {
            let mut row = vec![byte; BUFFER_BYTES / 2 - 1];
            row.push(b'\n');
            row
        })
        .collect()
}

#[test]
fn borrowed_rows_use_original_handle_preserve_locator_opens_and_owned_byte_parity() {
    let fixture = Fixture::new();
    let platform = OpeningPlatform {
        inner: TestPlatform::new(),
        locator_opens: Arc::default(),
    };
    let expected = multiple_buffers();
    let mut publication = prepared_rows(&fixture, platform.clone(), &expected);
    publication.publish_once(deadline()).expect("publish");
    let original = handle_key(&publication.frozen.owned.as_ref().unwrap().original_file);
    let mut owned_accounting = None;
    for borrow_original in [false, true, true] {
        platform.locator_opens.store(0, Ordering::SeqCst);
        let opens = Cell::new(0);
        let read_bytes = Cell::new(0);
        let process_bytes = Cell::new(0);
        let work = |unit| {
            match unit {
                Work::Open => opens.set(opens.get() + 1),
                Work::Read(bytes) => read_bytes.set(read_bytes.get() + bytes),
                Work::Process(bytes) => process_bytes.set(process_bytes.get() + bytes),
                Work::Write(_) => panic!("a row pass cannot write"),
                _ => {}
            }
            Ok(())
        };
        let mut rows = if borrow_original {
            publication.begin_borrowed_rows_with_context(deadline(), &work)
        } else {
            publication.begin_rows_with_context(deadline(), &work)
        }
        .expect("cursor");
        let cursor_handle = handle_key(rows.reader.get_ref().file());
        assert_eq!(cursor_handle == original, borrow_original);
        assert_eq!(platform.locator_opens.load(Ordering::SeqCst), 2);
        for row in &expected {
            assert_eq!(
                rows.next_with_context(deadline(), &work).unwrap(),
                Some(row.as_slice())
            );
        }
        assert_eq!(rows.next_with_context(deadline(), &work).unwrap(), None);
        assert!(rows.complete, "None requires full chain and EOF proof");
        assert_eq!(platform.locator_opens.load(Ordering::SeqCst), 4);
        assert_eq!(opens.get(), if borrow_original { 4 } else { 5 });
        let accounting = (read_bytes.get(), process_bytes.get());
        if borrow_original {
            assert_eq!(Some(accounting), owned_accounting);
        } else {
            owned_accounting = Some(accounting);
        }
    }
    eprintln!(
        "BORROWED_CURSOR real_file=1 default_clones=1 borrowed_clones=0 locator_opens_per_pass=4 byte_parity=1 eof=1"
    );
}

#[test]
fn borrowed_rows_early_drop_releases_borrow_reseeks_and_never_writes_retained_evidence() {
    let fixture = Fixture::new();
    let mut publication = prepared(&fixture, TestPlatform::new());
    let stage = fixture.0.join(".source-map.retained.tmp");
    let before = fs::metadata(&stage).unwrap().modified().unwrap();
    {
        let mut rows = publication
            .begin_borrowed_rows_with_context(deadline(), &INERT)
            .unwrap();
        assert_eq!(
            rows.next_with_context(deadline(), &INERT).unwrap(),
            Some(b"alpha\n".as_slice())
        );
        assert!(!rows.complete);
    }
    assert_eq!(
        publication.disposition(),
        SourceImportRecordArtifactPublicationDisposition::Prepared
    );
    {
        let mut rows = publication
            .begin_borrowed_rows_with_context(deadline(), &INERT)
            .unwrap();
        assert_eq!(
            rows.next_with_context(deadline(), &INERT).unwrap(),
            Some(b"alpha\n".as_slice())
        );
        assert_eq!(rows.next_with_context(deadline(), &INERT).unwrap(), None);
    }
    drop(publication);
    assert_eq!(fs::read(&stage).unwrap(), b"alpha\n");
    assert_eq!(fs::metadata(&stage).unwrap().modified().unwrap(), before);
    assert!(!fixture.0.join("artifact.source-map.v1").exists());
}

#[test]
fn borrowed_rows_refusal_is_sticky_and_unknown_publication_never_gets_cursor() {
    let fixture = Fixture::new();
    let mut publication = prepared(&fixture, TestPlatform::new());
    assert!(matches!(
        publication
            .begin_borrowed_rows_with_context(deadline(), &|_| Err(ControlError::ReadCancelled)),
        Err(SourceImportRecordArtifactError::Context(
            ControlError::ReadCancelled
        ))
    ));
    {
        let mut rows = publication
            .begin_borrowed_rows_with_context(deadline(), &INERT)
            .unwrap();
        assert!(matches!(
            rows.next_with_context(deadline(), &|_| Err(ControlError::ReadCancelled)),
            Err(SourceImportRecordArtifactError::Context(
                ControlError::ReadCancelled
            ))
        ));
        assert!(matches!(
            rows.next_with_context(deadline(), &INERT),
            Err(SourceImportRecordArtifactError::Closed)
        ));
    }
    assert_unknown(
        publication.publish_once_using(deadline(), |_, _| Err(io::Error::other("unknown"))),
    );
    assert!(matches!(
        publication.begin_borrowed_rows_with_context(deadline(), &INERT),
        Err(SourceImportRecordArtifactError::Closed)
    ));
    drop(publication);
    assert_eq!(
        fs::read(fixture.0.join(".source-map.retained.tmp")).unwrap(),
        b"alpha\n"
    );
}

#[test]
fn borrowed_rows_changed_bytes_identity_and_late_locator_refusal_cannot_complete() {
    for changed in [
        b"bravo\n".as_slice(),
        b"alph".as_slice(),
        b"alpha\nbeta\n".as_slice(),
    ] {
        let fixture = Fixture::new();
        let mut publication = prepared(&fixture, TestPlatform::new());
        publication.publish_once(deadline()).unwrap();
        let mut rows = publication
            .begin_borrowed_rows_with_context(deadline(), &INERT)
            .unwrap();
        fs::write(fixture.0.join(".source-map.retained.tmp"), changed).unwrap();
        loop {
            match rows.next_with_context(deadline(), &INERT) {
                Ok(Some(_)) => {}
                Ok(None) => panic!("changed bytes completed"),
                Err(_) => break,
            }
        }
        assert!(!rows.complete);
        assert!(matches!(
            rows.next_with_context(deadline(), &INERT),
            Err(SourceImportRecordArtifactError::Closed)
        ));
    }
    for late_locator in [false, true] {
        let fixture = Fixture::new();
        let platform = TestPlatform::new();
        let mut publication = prepared(&fixture, platform.clone());
        publication.publish_once(deadline()).unwrap();
        let mut rows = publication
            .begin_borrowed_rows_with_context(deadline(), &INERT)
            .unwrap();
        assert!(
            rows.next_with_context(deadline(), &INERT)
                .unwrap()
                .is_some()
        );
        if late_locator {
            platform.final_fail.store(true, Ordering::SeqCst);
        } else {
            platform.replace_identity();
        }
        assert!(rows.next_with_context(deadline(), &INERT).is_err());
        assert!(!rows.complete);
    }
}

#[test]
fn borrowed_rows_external_alias_offset_skip_refuses_then_fresh_pass_reseeks() {
    let fixture = Fixture::new();
    let expected = multiple_buffers();
    let mut publication = prepared_rows(&fixture, TestPlatform::new(), &expected);
    // Deliberate adversarial alias made before the exclusive cursor borrow. On
    // Windows and Unix File::try_clone shares the original object's position.
    let mut alias = publication
        .frozen
        .owned
        .as_ref()
        .unwrap()
        .original_file
        .try_clone()
        .unwrap();
    {
        let mut rows = publication
            .begin_borrowed_rows_with_context(deadline(), &INERT)
            .unwrap();
        assert_eq!(
            rows.next_with_context(deadline(), &INERT).unwrap(),
            Some(expected[0].as_slice())
        );
        alias.seek(SeekFrom::End(0)).unwrap();
        loop {
            match rows.next_with_context(deadline(), &INERT) {
                Ok(Some(_)) => {}
                Ok(None) => panic!("shared-position skip cannot complete a chain"),
                Err(_) => break,
            }
        }
        assert!(!rows.complete);
        assert!(matches!(
            rows.next_with_context(deadline(), &INERT),
            Err(SourceImportRecordArtifactError::Closed)
        ));
    }
    let mut rows = publication
        .begin_borrowed_rows_with_context(deadline(), &INERT)
        .unwrap();
    for row in &expected {
        assert_eq!(
            rows.next_with_context(deadline(), &INERT).unwrap(),
            Some(row.as_slice())
        );
    }
    assert_eq!(rows.next_with_context(deadline(), &INERT).unwrap(), None);
    eprintln!("BORROWED_CURSOR alias_offset_skip_refused=1 sequential_reseek=1 native_ctx=0");
}
