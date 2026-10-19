# search-source-identity — source and replacement map

## Current APIs and caller reality

| Current area | Actual caller/disposition |
|---|---|
| NTFS volume/file/directory identity | Native source acquisition uses the exact held-handle observations. Keep the small physical-key calculation or move it to the native source owner. |
| `derive_canonical_path_key` / `validate_identity_observation` | Native acquisition and old source-v2 registry use them. Replace the large observation/profile wrapper with one current root-relative locator + optional physical key. |
| `resolve_identity` | External production caller found in old `search-source-registry::source_v2`; otherwise package batch/tests. Replace with direct current-row lookup/reconcile. |
| binding/history transitions | Package tests/docs; no connected product caller found. Delete. |
| repository lineage/workspace identity | Package tests/docs; no connected producer/consumer found. Delete. |
| batch/current/recorded/redaction | Internal/package fixtures or old registry shapes. Delete. |
| legacy/Git digest profiles | Old compatibility/helper code. Exact Git OID and selected tree/path are sufficient revision evidence. |

## Small matching operation

The source owner may implement the following directly:

```text
match_current_source(
  root_id,
  current_locator,
  optional_physical_key,
  current_rows,
) -> SameSource | ProvenRename | NewSource | Conflict
```

Rules:

1. Existing row at the same locator is the same logical source unless an explicit selected policy says a
   replacement creates a new source; the physical-key change remains observable.
2. A missing old locator plus exact matching physical key at one new locator may be a rename.
3. A physical key simultaneously observed at several locators is a hardlink/alias case; apply one explicit
   finite policy and never index the same physical bytes repeatedly by accident.
4. Path normalization follows the registered root's actual lookup behavior. It cannot merge physical objects.
5. Content digest belongs to the revision. It is never source identity or rename proof.
6. Missing/ambiguous physical evidence never silently matches another source.
7. A new source ID is allocated/persisted by the current source transaction; it need not be a deterministic
   digest over every observation.

### Native physical key

For NTFS, use exact observed values from the held final handle:

```text
volume identity
file index
creation time
```

Keep full-width values. Do not add the root path, policy, source content, access membership or caller receipt.
Creation time protects against common file-index reuse; uncertainty remains explicit. Other filesystems use
a selected adapter only when real stable IDs are available.

### Git source and revision

A registered Git root/view defines the repository boundary. The current logical source is the exact path in
the selected tree. Revision/evidence binds:

```text
exact commit/tree snapshot
exact path
exact blob OID and retained bytes
```

Do not derive another `git-stable-identity` SHA-256 over a hard-coded 20-byte OID. Git repositories may use
different object formats; the Git adapter already knows the exact OID bytes/algorithm. Remote URL/name/ref
and guessed lineage do not enter source identity.

## Current-state integration

```text
inventory observation
→ native/Git adapter validates locator and optional physical key
→ match current source rows
→ one stable read / immutable ObjectRef
→ compare/set SourceRecord current RevisionRef and locator/physical key
→ publication later builds immutable ViewManifestEntry
```

No separate path-binding transaction/history, namespace owner generation or identity receipt is written.
Old live query evidence remains exact through the pinned view, not source identity history.

## Delete with consumers

```text
batch.rs
binding.rs
current.rs
recorded.rs
redaction.rs
lineage.rs
resolution/evidence portions not used by the small matcher
legacy_digest.rs
git_digest.rs / GIT_DIGEST.md profile
observation canonical codecs and profile descriptors
old source-v2 identity lookup/batch/receipt fixtures
```

Move the small NTFS key and matching rules to the concrete source/native owner if no substantial independent
crate remains. Remove the package and its workspace/dependency/status entries rather than leave a facade.

## Verification

- same locator/edit keeps current source;
- proven rename updates locator without duplicate source/preparation;
- path replacement/ambiguous identity is explicit;
- hardlink policy avoids accidental duplicate indexing;
- same bytes at different sources never merge;
- Git path/blob evidence uses the selected exact tree and repository object format;
- restart uses current rows and no binding/lineage replay;
- current identity data contains no access, corpus, ranking or receipt authority.
