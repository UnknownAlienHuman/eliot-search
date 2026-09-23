# Full security restriction restart records

T20 / #117, T21 prerequisite. Base `08c0477040a7d94288d61257aa1dbdb0e9317a01`.

`security_restriction` adds native persistence for the **complete** captured
restriction, not only the 124-byte policy metadata. One conditional redb
transaction writes the existing policy row and a version-1 operation row at
`eliot.control.access-restriction.v1\0<namespace bytes>`. No new table or database.

The restart row contains the original journal identity/generation, operation ID,
command digest, expected policy, complete expected/new security state, and every
required dependent. Each state preserves domain, policy/shadow/purge metadata,
full denied/purged membership sets, live snapshot digest and fail-closed flag.
It does not store a recursive chain of prior commands. The native ledger retains
its existing bounded idempotency history; only the latest domain command is kept.

The native operation ID is SHA-256 with a dedicated, length-framed domain over
installation/root/namespace/security-domain/operation identity. Mutable fields
are excluded so changed-input reuse conflicts rather than acquiring a new ID.
The supplied BLAKE3 command digest remains metadata; the existing journal also
fingerprints every actual class, byte, condition and generation independently.

New requests require a coherent native readback. Replacement additionally
requires a native predecessor commit matching the exact current domain command;
a valid-looking row without its operation receipt cannot be overwritten through
this API. Global generation and exact previous policy fence both writes. Initial
installation is explicit, requires an absent restart row, and cannot change an
existing metadata-only policy. Missing full state never means empty deny sets.

Same-owner recovery: read both rows, recover `readback.mutation()` through
`recover_security_restriction`, read again, then `confirm_current`. Recovery
writes nothing and recreates the original native command without refreshing its
generation/identity. `None` proves only operation absence, not unchanged expected
state or permission to retry. Later unrelated control writes are allowed; a newer
domain command makes an older receipt non-current even after policy-byte ABA.
Owner succession is deliberately **not** implicit: old-owner descriptors refuse.

Codec decoding rejects unknown version/magic, truncation, trailing bytes,
noncanonical/duplicate sets, invalid flags/UTF-8 and oversized lengths before
unbounded work. Inline commands are limited to 64 KiB and shared item/text bounds;
actual smaller journal limits also apply. Larger exact sets require a separately
qualified immutable-manifest path; they are never truncated or silently admitted.
Default debug output excludes domains, operations, membership sets and policies.

The access owner still validates restriction semantics. These disk types are not
`SecurityMutationEffects` completion, grants, live publication, invalidation
acknowledgements, or daemon cutover. Initialization requires externally established
authoritative state; never invoke it as automatic repair for missing recovery data.
After installation, policy updates must use the paired API; a standalone legacy
policy change is detected as an incoherent pair. Native owner-succession recovery,
manifest storage and concrete access/daemon composition remain open.

Eleven new tests cover native commit/reopen/recovery, exact replay/conflict,
stale-generation atomicity, missing receipts, malformed pairs, metadata-only
initialization, ABA, limits, cancellation, redaction and strict codec decoding.
A 440-byte known-answer fixture has an independently calculated Python SHA-256;
that reference calculation is not execution of the Rust encoder.

Verification in this environment: exact baseline/root-file preservation and
`git diff --check`; Rust compile/tests/rustfmt/Clippy **NOT_RUN**. The attempted
`cargo +1.98.0 check --locked -p search-control-redb --all-targets` exited 127
(Cargo absent). No Actions dispatch or independent acceptance. Required exact-head
follow-up: that check, package tests (including `security_restriction`), rustfmt,
strict Clippy and native recovery qualification. The local checkout is scope-only.
