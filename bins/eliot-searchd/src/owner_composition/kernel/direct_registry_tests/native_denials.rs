//! #578 Native denial proofs. Authored only; execution belongs to Root/#215.
use super::*;
use super::super::super::read_child::source_body_attempts_for_test;

fn registered() -> (Scratch, PathBuf, usize) {
    let s = Scratch::new(true);
    let policy = retained_v2(7);
    let ceiling = usize::try_from(policy.compiled().section().max_file_bytes()).unwrap();
    let archive = s.base.join("denial-policy.cbor");
    fs::write(&archive, policy.encoded_inputs()).unwrap();
    assert!(run_v2(&s, &s.source, &archive).unwrap().contains("\"registry_revision\":1"));
    (s, archive, ceiling)
}

fn assert_denied(
    s: &Scratch, archive: &Path, relative: &str, body: &[u8],
    operation: usize, reason: &str, expect_body: bool,
) {
    let path = s.source.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, body).unwrap();
    let args = [
        OsString::from("--admit-source-v2"), s.root.as_os_str().to_owned(),
        s.source.as_os_str().to_owned(), OsString::from(relative),
        archive.as_os_str().to_owned(), OsString::from(format!("{operation:032x}")),
        OsString::from("new"), OsString::from("--config-file"),
        s.config.as_os_str().to_owned(),
    ];
    let mut output = Vec::new();
    // TLS is cumulative and never reset; bracket only the real command under test.
    let before = source_body_attempts_for_test();
    let result = command::run(&args, Instant::now(), &mut output);
    let attempts = source_body_attempts_for_test().checked_sub(before).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(result.is_err(), "denial must fail the command: {output}");
    assert!(output.contains("\"admitted\":false"), "{output}");
    assert!(output.contains(reason), "{output}");
    assert!(!output.contains("\"admitted\":true"), "{output}");
    assert!(output.contains("\"ready\":false"), "{output}");
    assert!(output.contains("\"source_body_serving\":false"), "{output}");
    if expect_body {
        assert!(attempts > 0, "post-byte denial must actually read: {output}");
    } else {
        assert_eq!(attempts, 0, "pre-body denial attempted a body read: {output}");
    }
    for private in [relative, "denial-body-canary", s.source.to_str().unwrap()] {
        assert!(!output.contains(private), "denial exposed source data: {output}");
        assert!(!output.contains(&private.replace('\\', "\\\\")), "{output}");
    }
    // Native lifecycle/journal epochs may advance; semantic registry state must not.
    let metadata = run_v2(s, &s.source, &s.base.join("denial-policy.cbor")).unwrap();
    assert!(metadata.contains("\"registry_revision\":1"), "{metadata}");
    assert!(metadata.contains("\"root_count\":1"), "{metadata}");
}

#[test]
fn native_source_metadata_denials_attempt_no_body_and_publish_no_source() {
    let (s, archive, ceiling) = registered();
    assert!(ceiling > 0);
    let oversized = vec![b'x'; ceiling.checked_add(1).unwrap()];
    for (index, (relative, body)) in [
        ("tls.pem", b"denial-body-canary".as_slice()),
        (".env", b"denial-body-canary".as_slice()),
        ("generated/one.rs", b"denial-body-canary".as_slice()),
        ("vendor/one.rs", b"denial-body-canary".as_slice()),
        ("target/one.rs", b"denial-body-canary".as_slice()),
        (".cache/one.rs", b"denial-body-canary".as_slice()),
        // The evaluator explicitly contributes DenyEmptySource for size zero.
        ("empty.rs", b"".as_slice()),
        ("oversized.rs", oversized.as_slice()),
    ].into_iter().enumerate() {
        assert_denied(&s, &archive, relative, body, index + 1,
            "NATIVE_SOURCE_PRE_READ_REFUSED", false);
    }
}

#[test]
fn native_source_byte_denials_require_reads_and_publish_no_source() {
    let (s, archive, _) = registered();
    for (index, body) in [
        b"denial-body-canary\0".as_slice(), b"denial-body-canary\xff".as_slice(),
        b"-----BEGIN PRIVATE KEY-----\ndenial-body-canary\n".as_slice(),
        b"-----BEGIN ENCRYPTED PRIVATE KEY-----\r\ndenial-body-canary\n".as_slice(),
        b"-----BEGIN RSA PRIVATE KEY-----\ndenial-body-canary\n".as_slice(),
        b"-----BEGIN EC PRIVATE KEY-----\ndenial-body-canary\n".as_slice(),
        b"-----BEGIN DSA PRIVATE KEY-----\ndenial-body-canary\n".as_slice(),
        b"-----BEGIN OPENSSH PRIVATE KEY-----\ndenial-body-canary\n".as_slice(),
    ].into_iter().enumerate() {
        assert_denied(&s, &archive, "one.rs", body, index + 32,
            "NATIVE_SOURCE_POST_READ_REFUSED", true);
    }
}

#[test]
fn native_source_changed_root_policy_attempts_no_body_and_publishes_no_source() {
    let (s, _, _) = registered();
    let changed = s.base.join("changed-policy.cbor");
    fs::write(&changed, retained_v2(8).encoded_inputs()).unwrap();
    assert_denied(&s, &changed, "one.rs", b"denial-body-canary", 64,
        "SnapshotInvalid", false);
}
