//! Exact 17-line durable owner record codec and shape validation.

use search_runtime_owner::OwnerError;

use super::codec::{
    blake3_bytes, blake3_hex, hex, parse_digest32, parse_id16,
    parse_u32, parse_u64, push_field, push_line,
};
use super::spec::{
    DrainReasonText, LifecycleState, MAX_STATE_BYTES, OWNER_STATE_MAGIC,
};

/// Exact durable owner record: one monotone epoch bound to one installation,
/// one physical root, one executable and one process-creation token.
#[derive(Clone, Eq, PartialEq)]
pub(super) struct DurableOwnerRecord {
    pub(super) installation_id: [u8; 16],
    pub(super) installation_incarnation_id: [u8; 16],
    pub(super) data_root_id: [u8; 16],
    pub(super) epoch: u64,
    pub(super) previous_epoch: u64,
    pub(super) previous_record_digest: [u8; 32],
    pub(super) canonical_path_digest: [u8; 32],
    pub(super) volume_identity_digest: [u8; 32],
    pub(super) executable_digest: [u8; 32],
    pub(super) owner_token: [u8; 16],
    pub(super) owner_pid: u32,
    pub(super) lifecycle: LifecycleState,
    pub(super) drain_reason: DrainReasonText,
    pub(super) generation: u64,
    pub(super) record_digest: [u8; 32],
}

impl core::fmt::Debug for DurableOwnerRecord {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DurableOwnerRecord")
            .field("installation_id", &hex(&self.installation_id))
            .field(
                "installation_incarnation_id",
                &hex(&self.installation_incarnation_id),
            )
            .field("data_root_id", &hex(&self.data_root_id))
            .field("epoch", &self.epoch)
            .field("previous_epoch", &self.previous_epoch)
            .field("lifecycle", &self.lifecycle)
            .field("drain_reason", &self.drain_reason)
            .field("generation", &self.generation)
            .field("record_digest", &hex(&self.record_digest))
            .field("owner_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl DurableOwnerRecord {
    /// Canonical body bytes: magic plus every field line in fixed order.
    pub(super) fn encode_body(&self) -> Vec<u8> {
        let mut output = String::new();
        push_line(&mut output, OWNER_STATE_MAGIC);
        push_field(&mut output, "format_version", "1");
        push_field(&mut output, "installation_id", &hex(&self.installation_id));
        push_field(
            &mut output,
            "installation_incarnation_id",
            &hex(&self.installation_incarnation_id),
        );
        push_field(&mut output, "data_root_id", &hex(&self.data_root_id));
        push_field(&mut output, "epoch", &self.epoch.to_string());
        push_field(
            &mut output,
            "previous_epoch",
            &self.previous_epoch.to_string(),
        );
        push_field(
            &mut output,
            "previous_record_digest",
            &hex(&self.previous_record_digest),
        );
        push_field(
            &mut output,
            "canonical_path_digest",
            &hex(&self.canonical_path_digest),
        );
        push_field(
            &mut output,
            "volume_identity_digest",
            &hex(&self.volume_identity_digest),
        );
        push_field(
            &mut output,
            "executable_digest",
            &hex(&self.executable_digest),
        );
        push_field(&mut output, "owner_token", &hex(&self.owner_token));
        push_field(&mut output, "owner_pid", &self.owner_pid.to_string());
        push_field(&mut output, "lifecycle", self.lifecycle.as_str());
        push_field(&mut output, "drain_reason", self.drain_reason.as_str());
        push_field(&mut output, "generation", &self.generation.to_string());
        output.into_bytes()
    }

    /// Canonical bytes closed by the digest of all preceding body bytes.
    pub(super) fn encode(&self) -> Vec<u8> {
        let body = self.encode_body();
        let mut output = body.clone();
        output.extend_from_slice(b"record_digest=");
        output.extend_from_slice(blake3_hex(&body).as_bytes());
        output.push(b'\n');
        output
    }

    /// Strict canonical decode with exact digest readback.
    ///
    /// Any shape, value or digest disagreement quarantines; nothing is
    /// repaired or reinterpreted.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, OwnerError> {
        if bytes.len() > MAX_STATE_BYTES {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let text = core::str::from_utf8(bytes)
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        if !text.ends_with('\n') {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let lines: Vec<&str> = text.lines().collect();
        // Magic plus fifteen field lines plus the closing digest line.
        if lines.len() != 17 || lines[0] != OWNER_STATE_MAGIC {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let mut values: [Option<&str>; 15] = [None; 15];
        for line in &lines[1..16] {
            let Some((key, value)) = line.split_once('=') else {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            };
            let slot = match key {
                "format_version" => 0,
                "installation_id" => 1,
                "installation_incarnation_id" => 2,
                "data_root_id" => 3,
                "epoch" => 4,
                "previous_epoch" => 5,
                "previous_record_digest" => 6,
                "canonical_path_digest" => 7,
                "volume_identity_digest" => 8,
                "executable_digest" => 9,
                "owner_token" => 10,
                "owner_pid" => 11,
                "lifecycle" => 12,
                "drain_reason" => 13,
                "generation" => 14,
                _ => return Err(OwnerError::OwnerRecoveryQuarantined),
            };
            if values[slot].is_some() || value.is_empty() {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            }
            values[slot] = Some(value);
        }
        let digest_line = lines[16];
        let Some(digest_value) = digest_line.strip_prefix("record_digest=") else {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        };
        if values[0] != Some("1") {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let body_end = text
            .len()
            .checked_sub(digest_line.len() + 1)
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
        if blake3_hex(&text.as_bytes()[..body_end]) != digest_value {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let missing = || OwnerError::OwnerRecoveryQuarantined;
        let record = Self {
            installation_id: parse_id16(values[1])?,
            installation_incarnation_id: parse_id16(values[2])?,
            data_root_id: parse_id16(values[3])?,
            epoch: parse_u64(values[4])?,
            previous_epoch: parse_u64(values[5])?,
            previous_record_digest: parse_digest32(values[6])?,
            canonical_path_digest: parse_digest32(values[7])?,
            volume_identity_digest: parse_digest32(values[8])?,
            executable_digest: parse_digest32(values[9])?,
            owner_token: parse_id16(values[10])?,
            owner_pid: parse_u32(values[11])?,
            lifecycle: LifecycleState::parse(values[12].ok_or_else(missing)?)?,
            drain_reason: DrainReasonText::parse(values[13].ok_or_else(missing)?)?,
            generation: parse_u64(values[14])?,
            record_digest: parse_digest32(Some(digest_value))?,
        };
        record.validate_shape()?;
        Ok(record)
    }

    /// Internal consistency that one writer always produces.
    pub(super) fn validate_shape(&self) -> Result<(), OwnerError> {
        if self.epoch == 0 || self.generation == 0 {
            return Err(OwnerError::OwnerEpochMismatch);
        }
        if self.epoch == 1 {
            if self.previous_epoch != 0 || self.previous_record_digest != [0; 32] {
                return Err(OwnerError::OwnerEpochMismatch);
            }
        } else if self.previous_epoch != self.epoch - 1 {
            return Err(OwnerError::OwnerEpochMismatch);
        }
        match self.lifecycle {
            LifecycleState::Draining => {
                if self.drain_reason == DrainReasonText::None {
                    return Err(OwnerError::OwnerRecoveryQuarantined);
                }
            }
            LifecycleState::Active | LifecycleState::Released => {
                if self.drain_reason != DrainReasonText::None {
                    return Err(OwnerError::OwnerRecoveryQuarantined);
                }
            }
        }
        Ok(())
    }

    pub(super) fn refresh_digest(&mut self) {
        self.record_digest = blake3_bytes(&self.encode_body());
    }
}
