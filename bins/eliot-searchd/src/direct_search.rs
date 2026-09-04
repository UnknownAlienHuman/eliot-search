//! Bounded deterministic DIRECT scan over the current source-root catalog.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use crate::source_roots::SourceRootCatalog;

pub(crate) const MAX_QUERY_BYTES: usize = 4 * 1024;
pub(crate) const MAX_RESULT_LIMIT: usize = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectSearchLimits {
    pub(crate) max_directory_depth: usize,
    pub(crate) max_files_scanned: usize,
    pub(crate) max_file_bytes: usize,
    pub(crate) max_total_bytes: u64,
    pub(crate) max_relative_path_bytes: usize,
    pub(crate) max_preview_chars: usize,
}

impl DirectSearchLimits {
    pub(crate) const BASELINE: Self = Self {
        max_directory_depth: 32,
        max_files_scanned: 100_000,
        max_file_bytes: 8 * 1024 * 1024,
        max_total_bytes: 512 * 1024 * 1024,
        max_relative_path_bytes: 512,
        max_preview_chars: 120,
    };

    pub(crate) fn validate(self) -> Result<Self, DirectSearchError> {
        if self.max_directory_depth == 0
            || self.max_files_scanned == 0
            || self.max_file_bytes == 0
            || self.max_total_bytes == 0
            || self.max_relative_path_bytes == 0
            || self.max_preview_chars == 0
            || u64::try_from(self.max_file_bytes).ok() > Some(self.max_total_bytes)
        {
            return Err(DirectSearchError::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SearchCoverage {
    Complete,
    Partial,
}

impl SearchCoverage {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchHit {
    pub(crate) root_index: usize,
    pub(crate) relative_path: String,
    pub(crate) line_number: u64,
    pub(crate) preview: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectSearchResult {
    pub(crate) coverage: SearchCoverage,
    pub(crate) files_scanned: usize,
    pub(crate) bytes_scanned: u64,
    pub(crate) gaps: usize,
    pub(crate) total_matches: usize,
    pub(crate) hits: Vec<SearchHit>,
}

#[derive(Debug)]
struct SearchAccumulator {
    query_lower: String,
    result_limit: usize,
    limits: DirectSearchLimits,
    files_scanned: usize,
    bytes_scanned: u64,
    gaps: usize,
    total_matches: usize,
    hits: Vec<SearchHit>,
    partial: bool,
    hard_stop: bool,
}

pub(crate) fn direct_search(
    roots: &SourceRootCatalog,
    query: &str,
    result_limit: usize,
    limits: DirectSearchLimits,
) -> Result<DirectSearchResult, DirectSearchError> {
    let limits = limits.validate()?;
    if query.is_empty()
        || query.as_bytes().len() > MAX_QUERY_BYTES
        || query.chars().all(char::is_whitespace)
    {
        return Err(DirectSearchError::InvalidQuery);
    }
    if result_limit == 0 || result_limit > MAX_RESULT_LIMIT {
        return Err(DirectSearchError::InvalidResultLimit);
    }
    if roots.configured_count() == 0 {
        return Err(DirectSearchError::NoSourceRoots);
    }

    let unavailable = roots.unavailable_count();
    let mut accumulator = SearchAccumulator {
        query_lower: query.to_lowercase(),
        result_limit,
        limits,
        files_scanned: 0,
        bytes_scanned: 0,
        gaps: unavailable,
        total_matches: 0,
        hits: Vec::new(),
        partial: unavailable != 0,
        hard_stop: false,
    };
    for (root_index, root) in roots.available_paths() {
        walk_directory(root_index, root, root, 0, &mut accumulator);
        if accumulator.hard_stop {
            break;
        }
    }
    accumulator.hits.sort_by(|left, right| {
        left.root_index
            .cmp(&right.root_index)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
            .then_with(|| left.line_number.cmp(&right.line_number))
            .then_with(|| left.preview.cmp(&right.preview))
    });
    Ok(DirectSearchResult {
        coverage: if accumulator.partial {
            SearchCoverage::Partial
        } else {
            SearchCoverage::Complete
        },
        files_scanned: accumulator.files_scanned,
        bytes_scanned: accumulator.bytes_scanned,
        gaps: accumulator.gaps,
        total_matches: accumulator.total_matches,
        hits: accumulator.hits,
    })
}

fn walk_directory(
    root_index: usize,
    root: &Path,
    directory: &Path,
    depth: usize,
    accumulator: &mut SearchAccumulator,
) {
    if accumulator.hard_stop {
        return;
    }
    if depth > accumulator.limits.max_directory_depth {
        accumulator.record_gap();
        return;
    }
    let read_dir = match fs::read_dir(directory) {
        Ok(read_dir) => read_dir,
        Err(_) => {
            accumulator.record_gap();
            return;
        }
    };
    let mut entries = Vec::new();
    for entry in read_dir {
        match entry {
            Ok(entry) => entries.push(entry),
            Err(_) => accumulator.record_gap(),
        }
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        if accumulator.hard_stop {
            return;
        }
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                accumulator.record_gap();
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            accumulator.record_gap();
            continue;
        }
        let canonical = match fs::canonicalize(&path) {
            Ok(canonical) => canonical,
            Err(_) => {
                accumulator.record_gap();
                continue;
            }
        };
        if !canonical.starts_with(root) {
            accumulator.record_gap();
            continue;
        }
        if metadata.is_dir() {
            walk_directory(
                root_index,
                root,
                &canonical,
                depth.saturating_add(1),
                accumulator,
            );
        } else if metadata.is_file() {
            scan_file(root_index, root, &canonical, accumulator);
        }
    }
}

fn scan_file(
    root_index: usize,
    root: &Path,
    path: &Path,
    accumulator: &mut SearchAccumulator,
) {
    if accumulator.files_scanned >= accumulator.limits.max_files_scanned {
        accumulator.partial = true;
        accumulator.hard_stop = true;
        return;
    }
    accumulator.files_scanned += 1;

    let path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        _ => {
            accumulator.record_gap();
            return;
        }
    };
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            accumulator.record_gap();
            return;
        }
    };
    let opened_path = match fs::canonicalize(path) {
        Ok(opened_path) if opened_path == path && opened_path.starts_with(root) => opened_path,
        _ => {
            accumulator.record_gap();
            return;
        }
    };
    let before = match file.metadata() {
        Ok(metadata) if metadata.is_file() && same_file_identity(&path_metadata, &metadata) => {
            metadata
        }
        _ => {
            accumulator.record_gap();
            return;
        }
    };
    let file_limit = u64::try_from(accumulator.limits.max_file_bytes)
        .expect("bounded file ceiling fits u64");
    if before.len() > file_limit {
        accumulator.record_gap();
        return;
    }
    let Some(next_total) = accumulator.bytes_scanned.checked_add(before.len()) else {
        accumulator.partial = true;
        accumulator.hard_stop = true;
        return;
    };
    if next_total > accumulator.limits.max_total_bytes {
        accumulator.partial = true;
        accumulator.hard_stop = true;
        return;
    }

    let capacity = usize::try_from(before.len()).unwrap_or(accumulator.limits.max_file_bytes);
    let mut bytes = Vec::with_capacity(capacity);
    if file
        .by_ref()
        .take(file_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > accumulator.limits.max_file_bytes
    {
        accumulator.record_gap();
        return;
    }
    let after = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => {
            accumulator.record_gap();
            return;
        }
    };
    let final_path = match fs::canonicalize(path) {
        Ok(final_path) => final_path,
        Err(_) => {
            accumulator.record_gap();
            return;
        }
    };
    let final_path_metadata = match fs::metadata(&final_path) {
        Ok(metadata) => metadata,
        Err(_) => {
            accumulator.record_gap();
            return;
        }
    };
    if final_path != opened_path
        || !final_path.starts_with(root)
        || !same_file_snapshot(&before, &after)
        || !same_file_identity(&after, &final_path_metadata)
        || u64::try_from(bytes.len()).ok() != Some(before.len())
    {
        accumulator.record_gap();
        return;
    }
    accumulator.bytes_scanned = next_total;

    let Ok(text) = std::str::from_utf8(&bytes) else {
        accumulator.record_gap();
        return;
    };
    let relative = match opened_path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => {
            accumulator.record_gap();
            return;
        }
    };
    if relative.as_bytes().len() > accumulator.limits.max_relative_path_bytes {
        accumulator.record_gap();
        return;
    }

    for (index, line) in text.lines().enumerate() {
        if !line.to_lowercase().contains(&accumulator.query_lower) {
            continue;
        }
        let Some(next_matches) = accumulator.total_matches.checked_add(1) else {
            accumulator.partial = true;
            accumulator.hard_stop = true;
            return;
        };
        accumulator.total_matches = next_matches;
        if accumulator.hits.len() >= accumulator.result_limit {
            continue;
        }
        let line_number = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .unwrap_or(u64::MAX);
        let preview = line
            .chars()
            .take(accumulator.limits.max_preview_chars)
            .collect::<String>();
        accumulator.hits.push(SearchHit {
            root_index,
            relative_path: relative.clone(),
            line_number,
            preview,
        });
    }
}

fn same_file_snapshot(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    same_file_identity(before, after)
        && before.is_file()
        && after.is_file()
        && before.len() == after.len()
        && before.permissions().readonly() == after.permissions().readonly()
        && before.modified().ok() == after.modified().ok()
        && before.created().ok() == after.created().ok()
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

impl SearchAccumulator {
    fn record_gap(&mut self) {
        self.gaps = self.gaps.saturating_add(1);
        self.partial = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DirectSearchError {
    InvalidLimits,
    NoSourceRoots,
    InvalidQuery,
    InvalidResultLimit,
}

impl DirectSearchError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "DIRECT_SEARCH_INVALID_LIMITS",
            Self::NoSourceRoots => "SEARCH_NOT_CONFIGURED",
            Self::InvalidQuery => "SEARCH_QUERY_INVALID",
            Self::InvalidResultLimit => "SEARCH_RESULT_LIMIT_INVALID",
        }
    }
}

impl std::fmt::Display for DirectSearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DirectSearchError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_roots::SourceRootCatalog;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "eliot-search-direct-{name}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn returns_deterministic_line_hits() {
        let root = temporary_directory("hits");
        let source = root.join("source");
        fs::create_dir_all(source.join("nested")).expect("create source tree");
        fs::write(source.join("b.txt"), "needle B\n").expect("write b");
        fs::write(source.join("nested").join("a.txt"), "x\nNeedle A\n")
            .expect("write a");
        let catalog = SourceRootCatalog::load(
            root.join("state").join("source-roots.txt"),
            std::slice::from_ref(&source),
        )
        .expect("load catalog");

        let result = direct_search(&catalog, "needle", 10, DirectSearchLimits::BASELINE)
            .expect("search");
        assert_eq!(result.coverage, SearchCoverage::Complete);
        assert_eq!(result.total_matches, 2);
        assert_eq!(result.hits.len(), 2);
        assert_eq!(result.hits[0].relative_path, "b.txt");
        assert_eq!(result.hits[1].relative_path, "nested/a.txt");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unavailable_root_makes_coverage_partial() {
        let root = temporary_directory("partial");
        let source = root.join("source");
        fs::create_dir_all(&source).expect("create source");
        let config = root.join("state").join("source-roots.txt");
        SourceRootCatalog::load(config.clone(), std::slice::from_ref(&source))
            .expect("persist root");
        fs::remove_dir_all(&source).expect("remove source");
        let catalog = SourceRootCatalog::load(config, &[]).expect("reload missing root");

        let result = direct_search(&catalog, "needle", 10, DirectSearchLimits::BASELINE)
            .expect("partial search");
        assert_eq!(result.coverage, SearchCoverage::Partial);
        assert_eq!(result.gaps, 1);
        assert!(result.hits.is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
