//! Source fixtures only; native transport qualification belongs to #332/#215.
use super::*;

#[test]
fn expired_original_deadline_prevents_endpoint_open() {
    let endpoint =
        NativeEndpointNameV1::parse("eliot-search-v1-00112233-4455-6677-8899-aabbccddeeff")
            .unwrap();
    let deadline = Instant::now();
    assert!(matches!(
        NativeLocalConnection::open(endpoint, deadline),
        Err(NativeTransportError::DeadlineExpired)
    ));
}

#[test]
fn errors_and_io_failures_discard_raw_diagnostics() {
    for error in [
        NativeTransportError::UnsupportedPlatform,
        NativeTransportError::DeadlineExpired,
        NativeTransportError::Unavailable,
        NativeTransportError::UnsupportedPipe,
    ] {
        assert!(error.code().starts_with("NATIVE_TRANSPORT_"));
        assert_eq!(error.to_string(), error.code());
        assert!(std::error::Error::source(&error).is_none());
    }
    let error = read_result(Err(io::Error::other("private path/key sentinel"))).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(!format!("{error:?} {error}").contains("sentinel"));
    let error =
        write_result(Err(io::Error::other("private path/key sentinel")), false).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("sentinel"));
}

#[test]
fn empty_pipe_is_pending_and_closed_pipe_is_eof() {
    assert_eq!(
        read_result(Err(io::Error::from_raw_os_error(232)))
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock,
    );
    for code in [109, 233] {
        assert_eq!(
            read_result(Err(io::Error::from_raw_os_error(code))).unwrap(),
            0
        );
    }
    assert_eq!(read_result(Ok(3)).unwrap(), 3);
    assert_eq!(read_result(Ok(0)).unwrap(), 0);
}

#[test]
fn nonempty_zero_write_is_pending_and_partial_count_is_preserved() {
    assert_eq!(
        write_result(Ok(0), false).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(write_result(Ok(0), true).unwrap(), 0);
    assert_eq!(write_result(Ok(3), false).unwrap(), 3);
    assert_eq!(
        write_result(Err(io::Error::from_raw_os_error(232)), false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock,
    );
    for code in [109, 233] {
        assert_eq!(
            write_result(Err(io::Error::from_raw_os_error(code)), false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe,
        );
    }
}

#[cfg(windows)]
#[test]
fn endpoint_namespace_is_exactly_local_and_canonical() {
    let endpoint =
        NativeEndpointNameV1::parse("eliot-search-v1-00112233-4455-6677-8899-aabbccddeeff")
            .unwrap();
    assert_eq!(
        platform::endpoint_path(endpoint),
        r"\\.\pipe\eliot-search-v1-00112233-4455-6677-8899-aabbccddeeff",
    );
}
