# search-safe-reader — functions and integration

The [implementation guide](../../../docs/implementation/README.md) owns retention and preparation.
This package owns acquisition, not source history, a watcher or another immutable-object store.

## Existing source and callers

| Symbol or caller | Actual behavior |
|---|---|
| [`same_handle_read_core`](src/lib.rs) | Opens once, validates before/after metadata, reads the requested range, checks returned length and cancellation. The adapters supply physical observations; this core does not itself implement OS containment or hash content. |
| `safe_read`, `safe_read_with_cancel`, `acquire_with_cancel_v2` | Wrap the shared core with old barrier or root-purpose metadata/receipt validation. V2 is not a separate storage engine. |
| `safe_read_with_retries` | Reopens for a finite number of attempts after transient backend/handle-change failures. Cancellation and deterministic validation failures are not retried. |
| [`FinalHandleBackend`](../../../bins/eliot-searchd/src/safe_reader_adapter/kernel/backend.rs), [`read_full_file_via_kernel`](../../../bins/eliot-searchd/src/safe_reader_adapter/kernel/read.rs) | Existing daemon filesystem adapter and whole-file wrapper. The wrapper currently uses `never_cancel`; connect real cancellation when reusing its behavior. |
| [`FreshRootContext::acquire_source_v2`](../../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs) | Uses `acquire_with_cancel_v2`, obtains and hashes the body, then returns `AcquiredNativeSource` without that body. Transfer the same body to retention as required by the implementation guide. |
| [`parse_loose_object`](src/git.rs) | Parses already decompressed `<type> <size>\0payload`. It is not an ODB, pack/ref reader or verification that bytes hash to the requested Git OID. |
| `GitObjectBackend`, `read_git_object_no_execute` | Abstract loose-object acquisition, with only fake implementations found. The wrapper compares backend-supplied identities, parses and copies the payload; it does not independently verify the Git object hash. |
| [`plan_git_snapshot`](../../../bins/eliot-searchd/src/source_composition.rs) | Plans an already supplied object through old composition. No production repository reader calls it. Its repository+blob stable identity omits path occurrence; do not reuse that identity model for multiple paths/views. |

The earlier config, batch, containment-receipt and buffer-release logical function lists were proposals,
not implemented APIs. Retire receipt-only fields with their consumers; do not manufacture replacements.

## Filesystem byte handoff

Keep one selected-root read with actual length limits and before/after observations on the opened handle.
The walker supplies candidates, not verified contents. Pass the owned body and needed source observation
to the existing writer; preserve the source observation while committing its revision. Hash exact bytes
where content identity is established. Later storage reads verify their actual stored object when needed.
Do not add replay of local receipts or repeat stable reads solely to cross an internal function boundary.

Cancellation stops further work and returns no successful partial body. Source instability is a finite
retry or an explicit unavailable result. Decoding, normalization and evidence coordinates belong to
preparation; byte acquisition does not transform source truth.

## Concrete Git path

Use installed system Git as the first backend. Resolve the selected commit/tree to exact IDs once.
Run builtins with direct argument arrays for the selected repository, without a shell. Use raw plumbing:

- `ls-tree -r -z --full-tree <tree>` enumerates the initial committed view.
- `diff-tree --raw -r -z --no-renames --no-commit-id --no-ext-diff --no-textconv <old-tree> <new-tree>`
  finds changed paths and object IDs; unchanged subtrees require no blob reads.
- For an explicitly selected index view, `ls-files --stage -z` captures path/OID/stage entries. Conflict
  stages remain explicit. These blobs must not be labeled as current filesystem bytes.
- One `cat-file --batch-command --buffer -Z` process per indexing job, under the shared work budget,
  handles finite batches of `info <OID>` and `contents <OID>`. Check kind/size from `info` before requesting
  contents; issue `flush` for each finite batch. Read the framed length into the existing byte writer.

Every command uses the [controlled child boundary](#git-child-boundary) below. Direct argv and a read-only
builtin name alone do not prevent Git from executing configured helpers. Do not create a long-lived
process/supervisor for every registered root.

Raw blobs may be LFS pointer text, symlink contents or different bytes from a worktree after attribute/EOL
conversion. Preserve that view distinction. Treat submodules as separately selected repositories.
Git GC can remove unreferenced objects; retain the exact blobs needed for evidence in the existing
revision store. Missing/promised objects stay unavailable with lazy fetch disabled.

An OID returned by a lookup is not by itself a check of the returned bytes. At acquisition, verify the
expected Git object identity when relying on that identity and establish the retained content digest.
Use the repository's object hash kind; the current fixed 20-byte `GitObjectId` is old code, not a format
compatibility promise. No new signed-object or receipt layer is needed.

Verified donor: Git 2.51.0,
[`cat-file.c`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/builtin/cat-file.c)
streams raw blobs and queues commands until `flush`;
[`tree-diff.c`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/tree-diff.c)
skips matching OID/mode entries. See the same revision's
[command options](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/Documentation/git.adoc)
and [batch framing](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/Documentation/git-cat-file.adoc).
This pin is inspected evidence, not a promise that any installed Git supports every option.

## Git child boundary

Resolve one trusted installed executable outside indexed content; do not search an indexed working directory
for `git`. Pass fixed builtin arguments, selected native repository paths and validated OIDs, never a shell
string or request-supplied Git options. Ref resolution must separate user input from options.

Apply this prefix to every selected builtin, including index enumeration:

```text
git --no-pager --no-replace-objects --no-lazy-fetch --no-optional-locks -c core.fsmonitor=false ...
```

Keep external diff/textconv disabled on diff commands and omit filters, checkout and submodule recursion.
An unsupported required option fails explicitly; never retry with protections removed. These flags are a
selected command policy, not a sandbox or permission to run arbitrary Git commands.

Build a child-specific environment from approved OS necessities. Do not blindly inherit `GIT_CONFIG_*`,
`GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_INDEX_FILE`, object-directory/alternate, tracing, pager or
executable-search overrides. Set required routing explicitly from the admitted repository. Disable system/
global configuration for this child (`GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL` set to the platform null file),
terminal prompting and unintended pager/helper behavior. Repository/worktree config and referenced metadata
still need the selected admission policy; never bypass ownership checks with `safe.directory=*` or modify the
user's config. Linked worktrees, common directories and alternates outside admitted roots require explicit
admission or an unsupported result, not silent source-scope expansion.

The existing acquisition owner keeps the child, finite queues, stderr drain and execution deadline/cancellation
until exit is observed. Diagnostic bytes are untrusted: use the
[CLI display boundary](../../../bins/eliot-search/FUNCTIONS.md#untrusted-display-text), not raw passthrough.
Git acquisition receives execution-only state, not a Search view/scoring context. No generic runner is needed.

### Evidence and checks

At Git 2.51.0 revision `c44beea485f0f2feaf460e2ac87fdd5608d63cf0`,
[`ls-files`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/builtin/ls-files.c)
reads the index; [`post_read_index_from`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/read-cache.c)
invokes fsmonitor; [`query_fsmonitor_hook`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/fsmonitor.c)
launches the configured hook with `use_shell = 1`. A direct argv call by Eliot does not change that internal path.
[Git configuration documentation](https://git-scm.com/docs/git-config) describes environment overrides and
command-line precedence. These sources were checked on 2026-10-29; this is not a latest-version/security audit.

A disposable Linux fixture on that date used Git 2.47.3 and a harmless marker-writing fsmonitor hook.
`ls-files --stage -z` with the previously specified three `--no-*` flags exited 0 and invoked the hook once;
adding `-c core.fsmonitor=false` exited 0, returned identical entries and invoked it zero times. This is a donor
counterexample, not execution of an Eliot adapter or qualification of Git 2.51/Windows.

When implementing #335, check hook suppression, hostile inherited routing/config, unsupported flags, admitted
metadata roots, missing objects without fetch, bounded stderr and cancellation/reaping through actual consumers.
The corrected recipe introduces no new prerequisite for Foundation or plain-file Search.

## Native alternative, when packaging needs it

Choose gix instead if the product must work without an installed Git executable; do not implement both
paths preemptively. At donor revision `ad0202e3ab396d752a56664001c134f48d834177`,
[`gix-odb`](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-odb/src/lib.rs)
reads loose and packed local objects. `Repository::find_header` obtains type/size;
[`find_object`](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix/src/repository/object.rs)
loads the body into a reusable buffer. This is not a bounded streaming body API by itself.
Use borrowed `TreeRefIter` and the direct tree comparison described in
[reconciliation](../search-source-reconcile/FUNCTIONS.md), without similarity/attribute processing.
[`gix_object::compute_hash` / `compute_stream_hash`](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-object/src/lib.rs)
provide Git object identity computation; Eliot needs no bespoke loose/tree/pack parser.
