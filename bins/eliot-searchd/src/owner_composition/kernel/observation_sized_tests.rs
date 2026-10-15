use super::*;
use super::super::native_request::{NativeRootLimits, NativeRootRequest, NativeWork};
use search_runtime_owner::DataRootOpenMode;
use std::ffi::OsString;
use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

struct Scratch(PathBuf);
impl Scratch {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = temp.join(format!("owner-sized-image-{}-{stamp}-{}", std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("image"), bytes).unwrap();
        Self(root)
    }
    fn open(&self) -> fs::File { fs::File::open(self.0.join("image")).unwrap() }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert_eq!(fs::canonicalize(&self.0).unwrap(), self.0);
        assert_eq!(self.0.parent(), Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path()));
        assert!(self.0.file_name().unwrap().to_str().unwrap().starts_with("owner-sized-image-"));
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn sized_executable_reader_preserves_v1_and_admits_every_actual_read() {
    let bytes = (0..2 * EXECUTABLE_READ_BUFFER_BYTES + 12345)
        .map(|index| (index % 251) as u8).collect::<Vec<_>>();
    let scratch = Scratch::new(&bytes);
    let mut file = scratch.open();
    let mut legacy_reads = 0;
    let legacy = hash_executable_file(&mut file, |file, chunk| {
        legacy_reads += 1;
        file.read(chunk).map_err(|_| OwnerError::DataRootInvalid)
    }).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let request = NativeRootRequest::new(DataRootOpenMode::InspectExistingReadOnly,
        &[OsString::from("sized-image-fixture"), scratch.0.as_os_str().to_owned()],
        Instant::now() + Duration::from_secs(30), Arc::new(AtomicBool::new(false)),
        // One original entropy seam and four data/EOF read attempts.
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, bytes.len() + 1, 5, 1]).unwrap(),
    ).unwrap();
    let mut work = NativeWork::new();
    work.read_capacity(&request, EXECUTABLE_READ_BUFFER_BYTES).unwrap();
    let mut buffer = vec![0; EXECUTABLE_READ_BUFFER_BYTES];
    let mut requested = Vec::new();
    let digest = hash_executable_file_sized(&mut file, bytes.len() as u64, &mut buffer,
        |file, chunk| {
            work.read(&request, chunk.len())?;
            requested.push(chunk.len());
            work.call(&request, || file.read(chunk).map_err(|_| OwnerError::DataRootInvalid))
        }).unwrap();
    assert_eq!(digest, legacy, "chunk size does not alter the frozen v1 transcript");
    assert_eq!(requested, [EXECUTABLE_READ_BUFFER_BYTES, EXECUTABLE_READ_BUFFER_BYTES, 12345, 1]);
    assert!(legacy_reads > requested.len());
    let mut dispatched = false;
    assert_eq!(work.call(&request, || { dispatched = true; Ok(()) }), Err(OwnerError::ContractExhausted));
    assert!(!dispatched, "the helper cannot renew the original dispatch allowance");
}

#[test]
fn sized_executable_reader_keeps_short_reads_length_checks_and_first_refusal() {
    let bytes = b"unchanged-v1-image";
    let scratch = Scratch::new(bytes);
    let mut file = scratch.open();
    let expected = hash_executable_file(&mut file, |file, chunk|
        file.read(chunk).map_err(|_| OwnerError::DataRootInvalid)).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut buffer = [0; 7];
    let mut short_reads = 0;
    let actual = hash_executable_file_sized(&mut file, bytes.len() as u64, &mut buffer, |file, chunk| {
        short_reads += 1;
        let count = chunk.len().min(3);
        file.read(&mut chunk[..count]).map_err(|_| OwnerError::DataRootInvalid)
    }).unwrap();
    assert_eq!(actual, expected);
    assert!(short_reads > 3);

    file.seek(SeekFrom::Start(0)).unwrap();
    assert_eq!(hash_executable_file_sized(&mut file, bytes.len() as u64 + 1, &mut buffer,
        |file, chunk| file.read(chunk).map_err(|_| OwnerError::DataRootInvalid)),
        Err(OwnerError::OwnerGuardMismatch), "premature EOF cannot publish a prefix hash");
    file.seek(SeekFrom::Start(0)).unwrap();
    assert_eq!(hash_executable_file_sized(&mut file, bytes.len() as u64 - 1, &mut buffer,
        |file, chunk| file.read(chunk).map_err(|_| OwnerError::DataRootInvalid)),
        Err(OwnerError::OwnerGuardMismatch), "the one-byte EOF probe detects growth");
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut attempts = 0;
    assert_eq!(hash_executable_file_sized(&mut file, bytes.len() as u64, &mut buffer, |file, chunk| {
        attempts += 1;
        if attempts == 2 { return Err(OwnerError::OwnerCancelledBeforeMutation); }
        file.read(chunk).map_err(|_| OwnerError::DataRootInvalid)
    }), Err(OwnerError::OwnerCancelledBeforeMutation));
    assert_eq!(attempts, 2, "no further read after caller refusal");
}

#[test]
fn sized_executable_reader_rejects_invalid_bounds_before_dispatch() {
    let scratch = Scratch::new(b"");
    let mut file = scratch.open();
    let mut attempted = false;
    let mut read = |_: &mut fs::File, _: &mut [u8]| { attempted = true; Ok(0) };
    assert_eq!(hash_executable_file_sized(&mut file, 0, &mut [], &mut read), Err(OwnerError::DataRootInvalid));
    assert_eq!(hash_executable_file_sized(&mut file, MAX_EXECUTABLE_BYTES + 1, &mut [0; 1], &mut read),
        Err(OwnerError::DataRootInvalid));
    assert!(!attempted);
    assert_eq!(hash_executable_file_sized(&mut file, 1, &mut [0; 1], |_, _| Ok(2)),
        Err(OwnerError::DataRootInvalid), "a malformed adapter cannot overrun scratch");
}
