//! Closed observation-root failure vocabulary.

use std::io;

#[derive(Debug)]
pub enum SourceRootError {
    RootLimitExceeded,
    RootNotFound,
    RootNotDirectory,
    RootOverlap,
    DataRootOverlap,
    RootPathNotAbsolute,
    RootPathNotUtf8,
    InvalidRootPath,
    InvalidConfigPath,
    ConfigTooLarge,
    ConfigNotUtf8,
    SymlinkDenied,
    CatalogCorrupt,
    UpdateOutcomeUnknown,
    RootIo(io::Error),
    ConfigIo(io::Error),
}

impl SourceRootError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::RootLimitExceeded => "SOURCE_ROOT_LIMIT",
            Self::RootNotFound => "SOURCE_ROOT_NOT_FOUND",
            Self::RootNotDirectory => "SOURCE_ROOT_NOT_DIRECTORY",
            Self::RootOverlap => "SOURCE_ROOT_OVERLAP",
            Self::DataRootOverlap => "SOURCE_ROOT_DATA_ROOT_OVERLAP",
            Self::RootPathNotAbsolute => "SOURCE_ROOT_PATH_NOT_ABSOLUTE",
            Self::RootPathNotUtf8 => "SOURCE_ROOT_PATH_NOT_UTF8",
            Self::InvalidRootPath => "SOURCE_ROOT_PATH_INVALID",
            Self::InvalidConfigPath => "SOURCE_ROOT_CONFIG_PATH_INVALID",
            Self::ConfigTooLarge => "SOURCE_ROOT_CONFIG_TOO_LARGE",
            Self::ConfigNotUtf8 => "SOURCE_ROOT_CONFIG_NOT_UTF8",
            Self::SymlinkDenied => "SOURCE_ROOT_SYMLINK_DENIED",
            Self::CatalogCorrupt => "SOURCE_ROOT_CATALOG_CORRUPT",
            Self::UpdateOutcomeUnknown => "SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN",
            Self::RootIo(_) => "SOURCE_ROOT_IO_FAILED",
            Self::ConfigIo(_) => "SOURCE_ROOT_CONFIG_IO_FAILED",
        }
    }
}

impl core::fmt::Display for SourceRootError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootIo(error) | Self::ConfigIo(error) => Some(error),
            _ => None,
        }
    }
}
