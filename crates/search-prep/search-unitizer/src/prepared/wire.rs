//! Primitive framing for the complete exact-UTF8 artifact.

use crate::PreparationError;

pub(super) const MAGIC: &[u8; 8] = b"ELSPRP01";
pub(super) const VERSION: u16 = 1;
const BINDING_BYTES: usize = 16 + 16 + 8 + 2 + 1 + 32 + 8;
const SETTINGS_BYTES: usize = 2 + 2 + 4 + 4;
pub(super) const HEADER_BYTES: usize = MAGIC.len() + 2 + BINDING_BYTES + SETTINGS_BYTES + 8 + 8;
pub(super) const UNIT_BYTES: usize = 8 + 8 + 8 + 8 + 1 + 1;

pub(super) fn encoded_size(text: usize, units: usize) -> Result<usize, PreparationError> {
    units
        .checked_mul(UNIT_BYTES)
        .and_then(|size| size.checked_add(HEADER_BYTES))
        .and_then(|size| size.checked_add(text))
        .ok_or(PreparationError::OffsetOverflow)
}

pub(super) fn put_usize(out: &mut Vec<u8>, value: usize) -> Result<(), PreparationError> {
    let value = u64::try_from(value).map_err(|_| PreparationError::OffsetOverflow)?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], PreparationError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PreparationError::OffsetOverflow)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(PreparationError::InvalidArtifact)?;
        self.position = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PreparationError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PreparationError::InvalidArtifact)
    }

    pub(super) fn u16(&mut self) -> Result<u16, PreparationError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, PreparationError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, PreparationError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(super) fn usize(&mut self) -> Result<usize, PreparationError> {
        usize::try_from(self.u64()?).map_err(|_| PreparationError::OffsetOverflow)
    }

    pub(super) fn boolean(&mut self) -> Result<bool, PreparationError> {
        match self.take(1)? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(PreparationError::InvalidArtifact),
        }
    }

    pub(super) const fn at_end(&self) -> bool {
        self.position == self.bytes.len()
    }
}
