//! Authenticated bounded loopback endpoint client.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

const MAX_TOKEN_FILE_BYTES: usize = 4096;
const MIN_TOKEN_BYTES: usize = 32;
const MAX_CHALLENGE_LINE_BYTES: usize = 256;
const MAX_RESPONSE_LINE_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_LINES: usize = 1_000_000;
const IO_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn invoke_remote(
    address: &str,
    token_file: &Path,
    command: &str,
) -> Result<(), String> {
    if command.is_empty()
        || command.len() > 128 * 1024
        || command.contains('\n')
        || command.contains('\r')
    {
        return Err("REMOTE_COMMAND_INVALID".to_owned());
    }
    let address = address
        .parse::<SocketAddr>()
        .map_err(|_| "REMOTE_ADDRESS_INVALID".to_owned())?;
    if !address.ip().is_loopback() {
        return Err("REMOTE_NON_LOOPBACK_DENIED".to_owned());
    }
    let token_verifier = read_token_verifier(token_file)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(10))
        .map_err(|error| format!("REMOTE_CONNECT_ERROR:{error}"))?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
        .map_err(|error| format!("REMOTE_TIMEOUT_CONFIGURATION_ERROR:{error}"))?;
    let read_stream = stream
        .try_clone()
        .map_err(|error| format!("REMOTE_STREAM_CLONE_ERROR:{error}"))?;
    let mut reader = BufReader::new(read_stream);

    let challenge_line = read_bounded_line(&mut reader, MAX_CHALLENGE_LINE_BYTES)?
        .ok_or_else(|| "REMOTE_CHALLENGE_MISSING".to_owned())?;
    let challenge = challenge_line
        .strip_prefix("CHALLENGE\t")
        .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())
        .and_then(Digest::from_hex)?;
    let response = derive_response(token_verifier, challenge);
    write_line(&mut stream, &format!("AUTH\t{}", response.hex()))
        .map_err(|error| format!("REMOTE_AUTH_WRITE_ERROR:{error}"))?;
    let authenticated = read_bounded_line(&mut reader, MAX_RESPONSE_LINE_BYTES)?
        .ok_or_else(|| "REMOTE_AUTH_RESPONSE_MISSING".to_owned())?;
    if !authenticated.contains("\"event\":\"authenticated\"") {
        return Err("REMOTE_AUTHENTICATION_FAILED".to_owned());
    }

    write_line(&mut stream, command)
        .map_err(|error| format!("REMOTE_COMMAND_WRITE_ERROR:{error}"))?;
    let mut terminal_seen = false;
    let mut failed = false;
    for _ in 0..MAX_RESPONSE_LINES {
        let line = read_bounded_line(&mut reader, MAX_RESPONSE_LINE_BYTES)?
            .ok_or_else(|| "REMOTE_RESPONSE_TRUNCATED".to_owned())?;
        println!("{line}");
        if line.contains("\"event\":\"request_complete\"") {
            terminal_seen = true;
            failed = line.contains("\"ok\":false");
            break;
        }
    }
    if !terminal_seen {
        return Err("REMOTE_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned());
    }
    if failed {
        Err("REMOTE_REQUEST_FAILED".to_owned())
    } else {
        Ok(())
    }
}

fn read_token_verifier(path: &Path) -> Result<Digest, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("REMOTE_TOKEN_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("REMOTE_TOKEN_FILE_INVALID".to_owned());
    }
    if metadata.len() > u64::try_from(MAX_TOKEN_FILE_BYTES).unwrap_or(u64::MAX) {
        return Err("REMOTE_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    let mut file = File::open(path)
        .map_err(|error| format!("REMOTE_TOKEN_OPEN_ERROR:{error}"))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .map_err(|_| "REMOTE_TOKEN_FILE_TOO_LARGE".to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(MAX_TOKEN_FILE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("REMOTE_TOKEN_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        bytes.fill(0);
        return Err("REMOTE_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    if end.saturating_sub(start) < MIN_TOKEN_BYTES {
        bytes.fill(0);
        return Err("REMOTE_TOKEN_TOO_SHORT".to_owned());
    }
    let verifier = digest(&bytes[start..end]);
    bytes.fill(0);
    Ok(verifier)
}

fn derive_response(token_verifier: Digest, challenge: Digest) -> Digest {
    let mut input = Vec::with_capacity(96);
    input.extend_from_slice(b"eliot-search/loopback-response/v1\0");
    input.extend_from_slice(&token_verifier.0);
    input.extend_from_slice(&challenge.0);
    digest(&input)
}

fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    maximum_bytes: usize,
) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let mut limited = reader.take(
        u64::try_from(maximum_bytes.saturating_add(1)).unwrap_or(u64::MAX),
    );
    let read = limited
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("REMOTE_READ_ERROR:{error}"))?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > maximum_bytes || !bytes.ends_with(b"\n") {
        return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "REMOTE_FRAME_INVALID_UTF8".to_owned())
}

fn write_line(stream: &mut TcpStream, value: &str) -> io::Result<()> {
    stream.write_all(value.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Digest([u8; 32]);

impl Digest {
    fn from_hex(value: &str) -> Result<Self, String> {
        if value.len() != 64 {
            return Err("REMOTE_DIGEST_INVALID".to_owned());
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
        }
        Ok(Self(bytes))
    }

    fn hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("REMOTE_DIGEST_INVALID".to_owned()),
    }
}

fn digest(bytes: &[u8]) -> Digest {
    const INITIAL: [u32; 8] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a,
        0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
    ];
    const K: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5,
        0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
        0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
        0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
        0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc,
        0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
        0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7,
        0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
        0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
        0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
        0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3,
        0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
        0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5,
        0x391c_0cb3, 0x4ed8_aa4, 0x5b9c_ca4f, 0x682e_6ff3,
        0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
        0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
    ];

    fn compress(state: &mut [u32; 8], block: &[u8; 64], constants: &[u32; 64]) {
        let mut schedule = [0_u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            schedule[index] = u32::from_be_bytes(
                word.try_into().expect("four-byte SHA-256 schedule word"),
            );
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let first = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(constants[index])
                .wrapping_add(schedule[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut state = INITIAL;
    let mut blocks = bytes.chunks_exact(64);
    for block in &mut blocks {
        compress(
            &mut state,
            block
                .try_into()
                .expect("chunks_exact yields a complete SHA-256 block"),
            &K,
        );
    }
    let remainder = blocks.remainder();
    let mut tail = [0_u8; 128];
    tail[..remainder.len()].copy_from_slice(remainder);
    tail[remainder.len()] = 0x80;
    let padded = if remainder.len() < 56 { 64 } else { 128 };
    let bit_length = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    tail[padded - 8..padded].copy_from_slice(&bit_length.to_be_bytes());
    for block in tail[..padded].chunks_exact(64) {
        compress(
            &mut state,
            block
                .try_into()
                .expect("padded SHA-256 tail uses complete blocks"),
            &K,
        );
    }
    let mut output = [0_u8; 32];
    for (index, word) in state.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    Digest(output)
}
