//! Permanent, bounded initialization history. Decoding grants no native authority.
//!
//! The original request and historical completion are immutable. Later valid
//! owner generations and catalog appends are not compared with initial emptiness.

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalValue, DigestInputLimit,
    blake3_canonical_array_with_usage, parse_canonical_cbor, to_canonical_cbor,
};
use search_runtime_owner::OwnerError;

use super::codec::{blake3_bytes, hex};
use super::record::DurableOwnerRecord;
use super::spec::{DrainReasonText, INSTALLATION_MAGIC, LifecycleState};

pub(super) const JOURNAL_FILE: &str = ".eliot-search-initialization-journal.v2";
pub(super) const LAYOUT_PROFILE: &str = "legacy-direct-root-v3-journal-v2";
pub(super) const MAX_BODY: usize = 512 * 1024;
pub(super) const MAX_SEAL: usize = 32 * 1024;
pub(super) const MAX_JOURNAL: usize = 1024 * 1024;
const MAGIC: &[u8] = b"ELIOT-SEARCH-INITIALIZATION-JOURNAL-V2\n";
const SEAL_TAG: u8 = 1;
const REQUEST_DOMAIN: &str = "eliot/cbor/native-root-request/windows/v1";
const BODY_DOMAIN: &str = "eliot/cbor/data-root-initialization-journal/v2";
const NATIVE_DOMAIN: &str = "eliot/cbor/data-root-native-objects/v2";
const NAMESPACE_DOMAIN: &str = "eliot/cbor/data-root-initialization-namespace/v1";

/// Retained original inputs. A fresh replay invocation has a different request ID.
#[derive(Clone, Eq, PartialEq)]
pub(super) struct InitializationSubject {
    pub(super) original_request: [u8; 16],
    pub(super) request_digest: [u8; 32],
    pub(super) arguments: Vec<Vec<u8>>,
    pub(super) initialization: [u8; 16],
    pub(super) installation: [u8; 16],
    pub(super) incarnation: [u8; 16],
    pub(super) physical_root: [u8; 20],
    pub(super) root_id: [u8; 16],
    pub(super) path_digest: [u8; 32],
    pub(super) volume_digest: [u8; 32],
    pub(super) executable: [u8; 32],
    pub(super) namespace: [u8; 32],
    pub(super) owner_token: [u8; 16],
    pub(super) owner_pid: u32,
    pub(super) journal_identity: [u8; 20],
}

/// Historical facts, not current owner authority or an empty-catalog assertion.
#[derive(Clone, Eq, PartialEq)]
pub(super) struct InitializationCompletion {
    pub(super) subject_digest: [u8; 32],
    pub(super) native_objects: [[u8; 20]; 11],
    pub(super) native_digest: [u8; 32],
    pub(super) installation_bytes: Vec<u8>,
    pub(super) namespace_bytes: Vec<u8>,
    pub(super) log_bytes: Vec<u8>,
    pub(super) lifecycle_bytes: [Vec<u8>; 3],
    pub(super) selected_key_binding: [u8; 32],
}

pub(super) struct ParsedInitializationJournal {
    pub(super) subject: InitializationSubject,
    pub(super) completion: Option<InitializationCompletion>,
}

impl InitializationSubject {
    fn validate_input_sizes(&self) -> Result<(), OwnerError> {
        if self.arguments.len() != 3 {
            return Err(quarantine());
        }
        let mut total = 0_usize;
        for argument in &self.arguments {
            total = total.checked_add(argument.len()).ok_or_else(quarantine)?;
            if total > 256 * 1024 {
                return Err(quarantine());
            }
        }
        Ok(())
    }

    pub(super) fn validate(
        &self,
        charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
    ) -> Result<(), OwnerError> {
        self.validate_input_sizes()?;
        if self.original_request == [0; 16]
            || self.initialization == [0; 16]
            || self.installation == [0; 16]
            || self.incarnation == [0; 16]
            || self.root_id == [0; 16]
            || self.owner_token == [0; 16]
            || self.owner_pid == 0
            || self.physical_root == [0; 20]
            || self.journal_identity == [0; 20]
            || self.arguments.len() != 3
            || self.arguments[0] != native_literal("--initialize-data-root")
            || self.arguments[2] != native_literal(&hex(&self.initialization))
        {
            return Err(quarantine());
        }
        let mut total = 0_usize;
        for argument in &self.arguments {
            total = total.checked_add(argument.len()).ok_or_else(quarantine)?;
            if total > 256 * 1024
                || argument.is_empty()
                || argument.len() % 2 != 0
                || argument.as_chunks::<2>().0.contains(&[0, 0])
            {
                return Err(quarantine());
            }
        }
        let request_rows = std::iter::once(Ok(CanonicalValue::U64(2)))
            .chain(self.arguments.iter().map(|argument| bytes_value(argument)));
        if hash_rows(REQUEST_DOMAIN, request_rows, 4, MAX_BODY, charge)? != self.request_digest
            || initialization_namespace(
                self.initialization,
                self.installation,
                self.incarnation,
                self.root_id,
                self.executable,
                charge,
            )? != self.namespace
        {
            return Err(quarantine());
        }
        Ok(())
    }

    fn rows(&self) -> Result<Vec<CanonicalValue>, OwnerError> {
        self.validate_input_sizes()?;
        let arguments = array(
            self.arguments
                .iter()
                .map(|argument| bytes_value(argument))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        Ok(vec![
            CanonicalValue::U64(2),
            CanonicalValue::U64(3),
            bytes_value(b"windows")?,
            bytes_value(REQUEST_DOMAIN.as_bytes())?,
            CanonicalValue::U64(2),
            bytes_value(&self.original_request)?,
            bytes_value(&self.request_digest)?,
            arguments,
            bytes_value(&self.initialization)?,
            bytes_value(&self.installation)?,
            bytes_value(&self.incarnation)?,
            bytes_value(&self.physical_root)?,
            bytes_value(&self.root_id)?,
            bytes_value(&self.path_digest)?,
            bytes_value(&self.volume_digest)?,
            bytes_value(&self.executable)?,
            bytes_value(&self.namespace)?,
            bytes_value(&self.owner_token)?,
            CanonicalValue::U64(u64::from(self.owner_pid)),
            bytes_value(&self.journal_identity)?,
        ])
    }

    pub(super) fn digest(
        &self,
        charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
    ) -> Result<[u8; 32], OwnerError> {
        hash_rows(
            BODY_DOMAIN,
            self.rows()?.into_iter().map(Ok),
            20,
            MAX_BODY,
            charge,
        )
    }

    pub(super) fn frame(&self) -> Result<Vec<u8>, OwnerError> {
        let payload = encode(self.rows()?, MAX_BODY)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(MAGIC.len() + 4 + payload.len())
            .map_err(|_| OwnerError::ContractExhausted)?;
        output.extend_from_slice(MAGIC);
        append_frame(&mut output, &payload)?;
        Ok(output)
    }

    fn decode(bytes: &[u8]) -> Result<Self, OwnerError> {
        let mut rows = decode_rows(bytes, 20, MAX_BODY)?.into_iter();
        require_number(&mut rows, 2)?;
        require_number(&mut rows, 3)?;
        require_bytes(&mut rows, b"windows")?;
        require_bytes(&mut rows, REQUEST_DOMAIN.as_bytes())?;
        require_number(&mut rows, 2)?;
        let original_request = fixed(&mut rows)?;
        let request_digest = fixed(&mut rows)?;
        let argument_rows = take_array(next(&mut rows)?, 3)?;
        let arguments = argument_rows
            .into_iter()
            .map(|row| take_bytes(row, 256 * 1024))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            original_request,
            request_digest,
            arguments,
            initialization: fixed(&mut rows)?,
            installation: fixed(&mut rows)?,
            incarnation: fixed(&mut rows)?,
            physical_root: fixed(&mut rows)?,
            root_id: fixed(&mut rows)?,
            path_digest: fixed(&mut rows)?,
            volume_digest: fixed(&mut rows)?,
            executable: fixed(&mut rows)?,
            namespace: fixed(&mut rows)?,
            owner_token: fixed(&mut rows)?,
            owner_pid: u32::try_from(take_number(&next(&mut rows)?)?).map_err(|_| quarantine())?,
            journal_identity: fixed(&mut rows)?,
        })
    }
}

impl InitializationCompletion {
    fn validate_input_sizes(&self) -> Result<(), OwnerError> {
        if self.installation_bytes.len() > 1024
            || self.namespace_bytes.len() > 65
            || self.log_bytes.len() > 30
            || self.lifecycle_bytes.iter().any(|bytes| bytes.len() > 4096)
        {
            return Err(quarantine());
        }
        Ok(())
    }

    pub(super) fn validate(
        &self,
        subject: &InitializationSubject,
        charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
    ) -> Result<(), OwnerError> {
        self.validate_input_sizes()?;
        if self.subject_digest != subject.digest(charge)?
            || self.native_objects[0] != subject.physical_root
            || self.native_objects[10] != subject.journal_identity
            || self.native_objects.contains(&[0; 20])
            || self.native_digest != native_objects_digest(&self.native_objects, charge)?
            || self.installation_bytes != installation_bytes(subject, self.native_digest)
            || self.namespace_bytes != format!("{}\n", hex(&subject.namespace)).as_bytes()
            || self.log_bytes
                != format!("{}\n", search_source_registry::LEGACY_DIRECT_LOG_HEADER).as_bytes()
            || self.selected_key_binding == [0; 32]
        {
            return Err(quarantine());
        }
        let mut expected = DurableOwnerRecord::decode(&self.lifecycle_bytes[0])?;
        if expected.encode() != self.lifecycle_bytes[0]
            || expected.installation_id != subject.installation
            || expected.installation_incarnation_id != subject.incarnation
            || expected.data_root_id != subject.root_id
            || expected.canonical_path_digest != subject.path_digest
            || expected.volume_identity_digest != subject.volume_digest
            || expected.executable_digest != subject.executable
            || expected.owner_token != subject.owner_token
            || expected.owner_pid != subject.owner_pid
            || expected.epoch != 1
            || expected.previous_epoch != 0
            || expected.previous_record_digest != [0; 32]
            || expected.generation != 1
            || expected.lifecycle != LifecycleState::Active
            || expected.drain_reason != DrainReasonText::None
        {
            return Err(quarantine());
        }
        for (index, lifecycle) in [LifecycleState::Draining, LifecycleState::Released]
            .into_iter()
            .enumerate()
        {
            expected.lifecycle = lifecycle;
            expected.drain_reason = if lifecycle == LifecycleState::Draining {
                DrainReasonText::Shutdown
            } else {
                DrainReasonText::None
            };
            expected.generation += 1;
            expected.record_digest = blake3_bytes(&expected.encode_body());
            if DurableOwnerRecord::decode(&self.lifecycle_bytes[index + 1])? != expected
                || expected.encode() != self.lifecycle_bytes[index + 1]
            {
                return Err(quarantine());
            }
        }
        Ok(())
    }

    pub(super) fn frame(&self) -> Result<Vec<u8>, OwnerError> {
        self.validate_input_sizes()?;
        let native = array(
            self.native_objects
                .iter()
                .map(|identity| bytes_value(identity))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let rows = vec![
            CanonicalValue::U64(2),
            bytes_value(&self.subject_digest)?,
            native,
            bytes_value(&self.native_digest)?,
            bytes_value(&self.installation_bytes)?,
            bytes_value(&self.namespace_bytes)?,
            bytes_value(&self.log_bytes)?,
            bytes_value(&self.lifecycle_bytes[0])?,
            bytes_value(&self.lifecycle_bytes[1])?,
            bytes_value(&self.lifecycle_bytes[2])?,
            bytes_value(&self.selected_key_binding)?,
        ];
        let payload = encode(rows, MAX_SEAL)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(5 + payload.len())
            .map_err(|_| OwnerError::ContractExhausted)?;
        output.push(SEAL_TAG);
        append_frame(&mut output, &payload)?;
        Ok(output)
    }

    fn decode(bytes: &[u8]) -> Result<Self, OwnerError> {
        let mut rows = decode_rows(bytes, 11, MAX_SEAL)?.into_iter();
        require_number(&mut rows, 2)?;
        let subject_digest = fixed(&mut rows)?;
        let identities = take_array(next(&mut rows)?, 11)?
            .into_iter()
            .map(|row| take_bytes(row, 20)?.try_into().map_err(|_| quarantine()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            subject_digest,
            native_objects: identities.try_into().map_err(|_| quarantine())?,
            native_digest: fixed(&mut rows)?,
            installation_bytes: take_bytes(next(&mut rows)?, 1024)?,
            namespace_bytes: take_bytes(next(&mut rows)?, 65)?,
            log_bytes: take_bytes(next(&mut rows)?, 30)?,
            lifecycle_bytes: [
                take_bytes(next(&mut rows)?, 4096)?,
                take_bytes(next(&mut rows)?, 4096)?,
                take_bytes(next(&mut rows)?, 4096)?,
            ],
            selected_key_binding: fixed(&mut rows)?,
        })
    }
}

pub(super) fn decode_journal(
    bytes: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
) -> Result<ParsedInitializationJournal, OwnerError> {
    if bytes.len() > MAX_JOURNAL || !bytes.starts_with(MAGIC) {
        return Err(quarantine());
    }
    let mut remaining = &bytes[MAGIC.len()..];
    let subject = InitializationSubject::decode(take_frame(&mut remaining, MAX_BODY)?)?;
    subject.validate(charge)?;
    let completion = if remaining.is_empty() {
        None
    } else {
        if remaining.first() != Some(&SEAL_TAG) {
            return Err(quarantine());
        }
        remaining = &remaining[1..];
        let seal = InitializationCompletion::decode(take_frame(&mut remaining, MAX_SEAL)?)?;
        if !remaining.is_empty() {
            return Err(quarantine());
        }
        seal.validate(&subject, charge)?;
        Some(seal)
    };
    Ok(ParsedInitializationJournal {
        subject,
        completion,
    })
}

// Test cleanup needs the validated immutable subject even when initialization
// failed while appending its completion. Production journal decoding stays strict.
#[cfg(all(test, windows))]
pub(super) fn fixture_subject_namespace(bytes: &[u8]) -> Result<[u8; 32], OwnerError> {
    if bytes.len() > MAX_JOURNAL || !bytes.starts_with(MAGIC) { return Err(quarantine()); }
    let mut remaining = &bytes[MAGIC.len()..];
    let subject = InitializationSubject::decode(take_frame(&mut remaining, MAX_BODY)?)?;
    subject.validate(&mut |_| Ok(()))?;
    Ok(subject.namespace)
}

pub(super) fn initialization_namespace(
    initialization: [u8; 16],
    installation: [u8; 16],
    incarnation: [u8; 16],
    root: [u8; 16],
    executable: [u8; 32],
    charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
) -> Result<[u8; 32], OwnerError> {
    let rows = [
        initialization.to_vec(),
        installation.to_vec(),
        incarnation.to_vec(),
        root.to_vec(),
        executable.to_vec(),
    ]
    .into_iter()
    .map(|part| bytes_value(&part));
    hash_rows(NAMESPACE_DOMAIN, rows, 5, 1024, charge)
}

pub(super) fn native_objects_digest(
    identities: &[[u8; 20]; 11],
    charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
) -> Result<[u8; 32], OwnerError> {
    hash_rows(
        NATIVE_DOMAIN,
        identities.iter().map(|identity| bytes_value(identity)),
        11,
        4096,
        charge,
    )
}

pub(super) fn installation_bytes(
    subject: &InitializationSubject,
    native_digest: [u8; 32],
) -> Vec<u8> {
    format!("{INSTALLATION_MAGIC}\nformat_version=3\ninstallation_id={}\ninstallation_incarnation_id={}\ninitialization_id={}\nlayout_profile={LAYOUT_PROFILE}\nnative_objects_digest={}\n",
        hex(&subject.installation), hex(&subject.incarnation), hex(&subject.initialization),
        hex(&native_digest)).into_bytes()
}

fn hash_rows(
    domain: &str,
    rows: impl IntoIterator<Item = Result<CanonicalValue, OwnerError>>,
    count: usize,
    ceiling: usize,
    charge: &mut impl FnMut(usize) -> Result<(), OwnerError>,
) -> Result<[u8; 32], OwnerError> {
    let rows = rows.into_iter().collect::<Result<Vec<_>, _>>()?;
    if rows.len() != count {
        return Err(quarantine());
    }
    let value = array(rows)?;
    let payload = encode_value(&value, ceiling)?;
    let complete = domain
        .len()
        .checked_add(1)
        .and_then(|size| size.checked_add(payload.len()))
        .filter(|size| *size <= ceiling)
        .ok_or(OwnerError::ContractExhausted)?;
    // Reserve the exact complete preimage before hashing; failures never refund it.
    charge(complete)?;
    let output = blake3_canonical_array_with_usage(
        &CanonicalDigestDomain::parse(domain).map_err(|_| quarantine())?,
        count,
        take_array(value, count)?.into_iter().map(Ok),
        DigestInputLimit::new(ceiling).map_err(|_| quarantine())?,
    )
    .map_err(|_| quarantine())?;
    if output.complete_preimage_bytes() != complete {
        return Err(quarantine());
    }
    Ok(*output.digest().as_bytes())
}

fn encode(rows: Vec<CanonicalValue>, ceiling: usize) -> Result<Vec<u8>, OwnerError> {
    encode_value(&array(rows)?, ceiling)
}

fn encode_value(value: &CanonicalValue, ceiling: usize) -> Result<Vec<u8>, OwnerError> {
    let bytes = to_canonical_cbor(value).map_err(|_| quarantine())?;
    if bytes.as_slice().len() > ceiling {
        return Err(OwnerError::ContractExhausted);
    }
    Ok(bytes.into_vec())
}

fn decode_rows(
    bytes: &[u8],
    count: usize,
    ceiling: usize,
) -> Result<Vec<CanonicalValue>, OwnerError> {
    if bytes.len() > ceiling {
        return Err(quarantine());
    }
    take_array(
        parse_canonical_cbor(bytes).map_err(|_| quarantine())?,
        count,
    )
}

fn array(rows: Vec<CanonicalValue>) -> Result<CanonicalValue, OwnerError> {
    BoundedList::new(rows)
        .map(CanonicalValue::Array)
        .map_err(|_| quarantine())
}

fn bytes_value(bytes: &[u8]) -> Result<CanonicalValue, OwnerError> {
    BoundedBytes::new(bytes.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| quarantine())
}

fn take_array(value: CanonicalValue, count: usize) -> Result<Vec<CanonicalValue>, OwnerError> {
    let CanonicalValue::Array(rows) = value else {
        return Err(quarantine());
    };
    if rows.as_slice().len() != count {
        return Err(quarantine());
    }
    Ok(rows.into_vec())
}

fn take_bytes(value: CanonicalValue, ceiling: usize) -> Result<Vec<u8>, OwnerError> {
    let CanonicalValue::Bytes(bytes) = value else {
        return Err(quarantine());
    };
    if bytes.as_slice().len() > ceiling {
        return Err(quarantine());
    }
    Ok(bytes.into_vec())
}

const fn take_number(value: &CanonicalValue) -> Result<u64, OwnerError> {
    if let CanonicalValue::U64(number) = value {
        Ok(*number)
    } else {
        Err(quarantine())
    }
}

fn next(rows: &mut std::vec::IntoIter<CanonicalValue>) -> Result<CanonicalValue, OwnerError> {
    rows.next().ok_or_else(quarantine)
}

fn fixed<const N: usize>(
    rows: &mut std::vec::IntoIter<CanonicalValue>,
) -> Result<[u8; N], OwnerError> {
    take_bytes(next(rows)?, N)?
        .try_into()
        .map_err(|_| quarantine())
}

fn require_number(
    rows: &mut std::vec::IntoIter<CanonicalValue>,
    expected: u64,
) -> Result<(), OwnerError> {
    if take_number(&next(rows)?)? == expected {
        Ok(())
    } else {
        Err(quarantine())
    }
}

fn require_bytes(
    rows: &mut std::vec::IntoIter<CanonicalValue>,
    expected: &[u8],
) -> Result<(), OwnerError> {
    if take_bytes(next(rows)?, expected.len())? == expected {
        Ok(())
    } else {
        Err(quarantine())
    }
}

fn append_frame(output: &mut Vec<u8>, payload: &[u8]) -> Result<(), OwnerError> {
    let length = u32::try_from(payload.len()).map_err(|_| OwnerError::ContractExhausted)?;
    let size = output
        .len()
        .checked_add(4)
        .and_then(|size| size.checked_add(payload.len()))
        .filter(|size| *size <= MAX_JOURNAL)
        .ok_or(OwnerError::ContractExhausted)?;
    output
        .try_reserve_exact(size - output.len())
        .map_err(|_| OwnerError::ContractExhausted)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(payload);
    Ok(())
}

fn take_frame<'a>(bytes: &mut &'a [u8], ceiling: usize) -> Result<&'a [u8], OwnerError> {
    let length_bytes: [u8; 4] = bytes
        .get(..4)
        .ok_or_else(quarantine)?
        .try_into()
        .map_err(|_| quarantine())?;
    let length = usize::try_from(u32::from_be_bytes(length_bytes)).map_err(|_| quarantine())?;
    let end = 4_usize.checked_add(length).ok_or_else(quarantine)?;
    if length == 0 || length > ceiling {
        return Err(quarantine());
    }
    let frame = bytes.get(4..end).ok_or_else(quarantine)?;
    *bytes = &bytes[end..];
    Ok(frame)
}

fn native_literal(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

const fn quarantine() -> OwnerError {
    OwnerError::OwnerRecoveryQuarantined
}

#[cfg(test)]
#[path = "initialization_journal_tests.rs"]
mod tests;
