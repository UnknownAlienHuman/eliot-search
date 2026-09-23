# Native access-policy journal boundary

T20 / #117. Base `4d9fdad35c8b2db8baa9771408032512f6420c56`, 2026-09-22.

`access_policy::AccessPolicyMutation` persists the existing 124-byte
`policy_codec::AccessPolicyRecord` through `PersistentControlJournal`'s
conditional transaction engine. There is no second database or new redb table.
The key is `eliot.control.access-policy.v1\0` followed by the 16 namespace bytes.

The descriptor freezes journal identity, operation ID, original journal generation,
exact previous class/bytes (or explicit absence), and replacement. Initialization
is not upsert. Namespace substitution is refused; identity includes owner epoch.
Actual configured journal limits, cancellation, possible-write fencing and native
receipt storage remain enforced by the existing engine. The supplied command digest
never substitutes for that engine's fingerprint of the complete actual request.

`commit_access_policy` returns an opaque `AccessPolicyCommit` only from native
commit/replay. `recover_access_policy` inspects the SAME descriptor without writing:
`Some` is a verified (possibly historical) commit, `None` is resolved absence,
errors remain conflicts/uncertainty/quarantine. No refresh of the operation ID,
expected generation or owner binding is implicit. Owner succession is separate.

`read_access_policy` performs a bounded, coherent administrative/recovery read,
not a query hot-path scan. Absence is explicit; wrong class, malformed encoding or
foreign embedded namespace fails closed. `confirm_commit` checks a native commit
capsule against that readback, including generation and exact replacement. Even an
ABA return to identical bytes cannot make an old receipt current. Neither commit
capsules nor readbacks authorize serving. The existing guarded snapshot publisher
must independently verify current disk state before publication.

Policy semantics/monotonicity stay in `search-access`. This is NOT the complete
`SecurityMutationEffects` adapter: full deny/purge membership sets, the complete
`SecurityRestriction` restart record, dependent acknowledgements and daemon wiring
still require their owning codecs/composition. Never reconstruct these from the
policy digest or treat this metadata row as a grant or accepted live restriction.

Eight new native temporary-redb test functions cover initialization/replacement,
reopen/recovery, exact preconditions, replay/conflict, ABA, semantic corruption,
owner mismatch, configured limits, cancellation and receipt correspondence. One
compile-fail example rejects construction of a verified capsule from an arbitrary
receipt. No prior tests, codec bytes, dependencies or workflows change.

Verification: exact baseline `src/lib.rs` Git blob matched; scoped diff and source
checks passed. `cargo +1.98.0 check --locked -p search-control-redb --all-targets`
exited 127 (`cargo: command not found`). Rust compilation, tests, rustfmt and Clippy
are NOT_RUN, not PASS. Full native/process qualification and independent review
remain outstanding. No Actions run or acceptance receipt was created.
