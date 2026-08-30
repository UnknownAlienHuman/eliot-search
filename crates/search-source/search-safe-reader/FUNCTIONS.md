# Function contract — `search-safe-reader`

**Status:** W2/P03 logical contract; no Windows final-handle or Git-object acquisition evidence exists.

This package owns bounded no-execute acquisition of exact source bytes from an already admitted local
root or an already identified local Git object. It owns no admission policy, source identity, registry,
revision storage, materialization or currentness claim.

## Global rules

- every request binds an admitted root/source/owner generation and an exact locator or Git object ID;
- root containment is checked on the final opened handle/object source, not only on path text;
- source bytes are returned only after bounded pre/read/post stability verification;
- hooks, filters, attributes requiring process execution, credential prompts, remote fetches and network
  are disabled;
- source bytes and unrestricted absolute paths never enter ordinary diagnostics, receipts or telemetry;
- cancellation, timeout or instability never yields a successful partial byte product;
- equal immutable input/observations produce equal digests and receipts.

## Configuration operations

### `section_descriptor() -> ConfigSectionDescriptor`

### `compiled_defaults() -> ConfigSectionInput`

### `validate_section(input, platform, accepted_capabilities) -> Result<ValidatedReaderConfig, ReadError>`

Implements `config/sections/source_reader.md`. The configured byte ceiling cannot exceed the admitted
source ceiling. Network, hooks and filters remain fixed false. Retry count/delay and size reductions are
bounded.

### `section_digest(validated) -> Blake3Digest32`

### `plan_section_change(old, new) -> Result<SectionReloadDecision, ReadError>`

Restrictive byte/retry changes may apply to future reads. Any increase beyond qualified/admission limits,
or any attempt to enable network/execution, is restart/gate/reject according to the section packet.

## Request and root validation

### `validate_read_request(request, registry_view, admission_receipt, owner_fence, limits) -> Result<ValidatedReadRequest, ReadError>`

Requires exact source/root/path-binding or Git-object identity, current root/registry/owner generations,
matching accepted admission receipt, finite byte/deadline/retry budget and explicit expected source kind.

A stale binding, foreign root, review/deny admission result or implicit disk-wide locator fails before
open. This operation performs no file read.

### `resolve_final_handle(locator, admitted_root, platform_port, deadline, cancel) -> Result<ResolvedSourceHandle, ReadError>`

Opens the source with the qualified no-follow/share semantics, resolves the final object and records
stable root/volume/file/reparse identity plus bounded metadata.

Path traversal, device/alternate-stream policy violation, reparse cycle or unresolved final identity is
rejected. Cancellation closes every opened handle and returns no reusable proof.

### `verify_final_containment(handle, admitted_root, platform_port) -> Result<ContainmentReceipt, ReadError>`

Proves the final object remains inside the exact admitted root under the qualified platform identity
profile. A symlink/reparse target outside the root is denied unless that target root is independently
admitted and named by the request.

Textual prefix, normalized-path prefix or initial path traversal is not sufficient containment evidence.

## Stable file acquisition

### `observe_before_read(handle, platform_port) -> Result<FileObservation, ReadError>`

Captures stable physical identity, size, timestamps/change identity where qualified, path-binding/root
fence and source kind. Missing load-bearing fields are explicit.

### `read_bounded(handle, byte_limit, buffer_policy, deadline, cancel) -> Result<ReadBuffer, ReadError>`

Reads at most the accepted ceiling into bounded process memory while computing the canonical source-byte
digest. Oversize input fails before unbounded allocation and returns no truncated success.

### `observe_after_read(handle, platform_port) -> Result<FileObservation, ReadError>`

Rechecks the same load-bearing identity/metadata after the final byte read and flushes no source state.

### `verify_pre_read_post(before, bytes, after, request) -> Result<StableBytes, ReadError>`

Requires exact physical/root/source identity and compatible size/change observations before/after, byte
length equal the observed stable size and digest over exactly returned bytes.

A material mismatch is `SOURCE_UNSTABLE`; it is not downgraded to a current/successful read.

### `read_stably(request, platform_port, clock, cancel) -> Result<StableReadReceipt, ReadError>`

Runs final-handle resolution, containment and bounded pre/read/post attempts. Retries only the exact
request within configured finite count/delay and closes every attempt handle.

Success returns process-memory bytes through a guarded product plus content-free receipt binding source,
root, owner/binding generations, observations, byte length/digest, encoding observation class and
attempt count. The package does not persist bytes.

## Git object acquisition

### `validate_git_object_request(request, repository_identity, workspace_view, limits) -> Result<ValidatedGitObjectRequest, ReadError>`

Requires exact admitted local repository/worktree identity, object ID, optional tree path, source/view
fence and object/byte limits. Branch name or HEAD alone is insufficient.

### `read_git_object_no_execute(request, git_object_port, deadline, cancel) -> Result<StableReadReceipt, ReadError>`

Reads the exact local object using an accepted no-process/no-network backend. Hooks, filters, smudge/
clean drivers, credential helpers, prompts, LFS/network fetch and repository-controlled commands are not
invoked.

The result binds repository/object/source/view identities, length and digest. Missing object, promised but
unavailable object or required remote fetch returns explicit unavailable—not an implicit network call.

## Encoding observation

### `observe_encoding(bytes, profile) -> EncodingObservation`

Returns bounded BOM/UTF validity/declared-encoding compatibility and reason metadata without changing
source-truth bytes. Decoding/materialization belongs to `search-materializer`.

## Batch acquisition

### `read_batch(requests, ports, limits, deadline, cancel) -> Result<StableReadBatch, ReadError>`

Canonicalizes a finite request set and returns one explicit outcome per item. Per-item bytes remain
separately guarded. Cancellation stops further work and marks all unprocessed items; it never labels the
batch complete or omits failures.

## Redaction and cleanup

### `redacted_read_view(receipt, disclosure) -> RedactedReadView`

Returns opaque source/root IDs, location class/path digest, byte count/digest, attempts and reason codes.
It excludes source bytes, unrestricted paths and foreign root/repository details.

### `release_read_product(product) -> ReadReleaseReceipt`

Drops/zeroizes package-owned transient buffers where the qualified allocator/runtime permits and closes
handles. It makes no physical-memory-erasure claim.

## Cancellation, deadline and crash semantics

All open/read/object operations have finite deadlines and cancellation. Cancellation before stable
verification produces no successful product. A process crash loses transient bytes/handles; no durable
recovery is owned here. Callers retry by reopening the exact request and must revalidate current root,
binding, admission and owner fences.

## Typed failures

- `READ_REQUEST_INVALID`
- `ROOT_NOT_ADMITTED`
- `ADMISSION_RECEIPT_STALE`
- `SOURCE_BINDING_STALE`
- `PATH_ESCAPES_ADMITTED_ROOT`
- `REPARSE_CYCLE`
- `DEVICE_OR_STREAM_PATH_DENIED`
- `SOURCE_UNSTABLE`
- `SOURCE_TOO_LARGE`
- `SOURCE_READ_TIMEOUT`
- `SOURCE_READ_CANCELLED`
- `SOURCE_ENCODING_UNSUPPORTED`
- `GIT_OBJECT_NOT_FOUND`
- `GIT_OBJECT_REQUIRES_NETWORK`
- `NO_EXECUTE_POLICY_DENIED`
- `READ_BATCH_INCOMPLETE`
- `READ_PLATFORM_IDENTITY_UNAVAILABLE`

## Required tests / qualification evidence

- final-handle containment versus textual-prefix adversarial fixtures;
- symlink/junction/reparse escape, cycle, replacement and alternate-stream/device denial;
- file changes before/during/after read become `SOURCE_UNSTABLE`;
- size ceiling enforced before unbounded allocation and no truncated success;
- finite retry/delay and cancellation closes all handles/buffers;
- exact byte length/digest and deterministic receipt goldens;
- Git exact object read with hooks/filters/prompts/network/credential helpers never invoked;
- promised/missing Git object returns explicit unavailable;
- stale admission/root/binding/owner/workspace fence rejected before read;
- encoding observation never mutates bytes or claims decoded fidelity;
- batch accounts every item under cancellation/failure;
- source bytes/path absent from Debug/errors/receipts/log/metric fixtures;
- `source_reader` configuration floors and change classification;
- fake platform/Git/clock/cancellation ports prove no registry/CAS/materializer ownership.
