//! Explicit file-owned locator hints. No registration, policy or currentness authority.
use super::{
    EffectiveDaemonConfig,
    spec::{key_name, section_name},
};
use search_config::ConfigValue;
use search_contracts::RootBindingId;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub struct RootLocatorHints(BTreeMap<RootBindingId, PathBuf>);
impl std::fmt::Debug for RootLocatorHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RootLocatorHints")
            .field("count", &self.0.len())
            .finish()
    }
}
const INVALID: &str = "SOURCE_ROOT_LOCATORS_INVALID";
impl RootLocatorHints {
    pub fn from_config(config: &EffectiveDaemonConfig) -> Result<Self, String> {
        let section = section_name("source_root_locators").map_err(|_| INVALID)?;
        let key = key_name("entries").map_err(|_| INVALID)?;
        let field = config
            .snapshot()
            .section(&section)
            .and_then(|v| v.field(&key))
            .ok_or(INVALID)?;
        let ConfigValue::StringList(entries) = &field.value else {
            return Err(INVALID.into());
        };
        Self::parse(entries)
    }
    pub(super) fn parse(entries: &[String]) -> Result<Self, String> {
        if entries.len() > 64 {
            return Err(INVALID.into());
        }
        let mut values = BTreeMap::new();
        for entry in entries {
            if entry.len() > 512 {
                return Err(INVALID.into());
            }
            let (id, path) = entry.split_once('=').ok_or(INVALID)?;
            if id.len() != 32
                || !id
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
                || path.contains('\0')
                || path.contains(['\r', '\n'])
            {
                return Err(INVALID.into());
            }
            let mut bytes = [0; 16];
            for (slot, pair) in bytes.iter_mut().zip(id.as_bytes().as_chunks::<2>().0) {
                let digit = |v: u8| if v <= b'9' { v - b'0' } else { v - b'a' + 10 };
                *slot = (digit(pair[0]) << 4) | digit(pair[1]);
            }
            let path = PathBuf::from(path);
            if !path.is_absolute()
                || path
                    .components()
                    .any(|v| matches!(v, Component::ParentDir | Component::CurDir))
                || values
                    .insert(RootBindingId::from_bytes(bytes), path)
                    .is_some()
            {
                return Err(INVALID.into());
            }
        }
        Ok(Self(values))
    }
    pub fn selected(&self, path: &Path) -> Result<RootBindingId, String> {
        path.to_str().ok_or(INVALID)?; // No lossy Windows conversion.
        let mut matches = self.0.iter().filter(|(_, value)| value.as_path() == path);
        let id = *matches.next().ok_or("SOURCE_ROOT_LOCATOR_UNAVAILABLE")?.0;
        if matches.next().is_some() {
            return Err(INVALID.into());
        }
        Ok(id)
    }
    pub fn get(&self, id: &RootBindingId) -> Result<&Path, String> {
        self.0
            .get(id)
            .map(PathBuf::as_path)
            .ok_or_else(|| "SOURCE_ROOT_LOCATOR_UNAVAILABLE".into())
    }
}
