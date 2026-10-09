# Indexed contract v1 — issue #258

`search_contracts::indexed` owns the provider-neutral S9.5/S10.3 values. It
performs no I/O and grants no source, access, publication or finish authority.
The planner, publication and bridge ports remain separate issues.

The sole ordered `PAYLOAD_FIELDS` table has 20 fields. Filtering its `indexed`
flag yields exactly 19 baseline indexes: 11 UUID, six keyword and two integer.
The full point-identity digest is readback evidence, not a baseline filter
index. Source membership, text, paths, vector digests and payload digests are
absent. Four optional fields are omitted when absent; explicit null is rejected.
Provider map iteration order does not change the admitted index set.

Epoch retains signed `i64` storage and is closed to `0..=2^53`. Zero is an empty
generation, published payload validity begins at one, and the maximum remains
readable with no next epoch. Open-ended validity omits its upper bound. Epochs
are compared only after collection generation equality. Stored values above
the new bound fail closed; this is a narrower admission contract, not an
in-place rewrite of old state.

The initial schema mode is explicitly sparse-only. Sparse vectors have a
nonzero logical index ceiling and required filtered-IDF modifier. Values are
finite, nonempty and capped; sparse indices must be strictly increasing and
below their ceiling. An explicit dense mode binds dimensions and distance to
the schema candidate. It does not enable or qualify a semantic worker. Multivectors,
quantization and vendor defaults are outside v1.

One bounded, nonempty canonical membership population supplies retrieval,
filtered IDF, count, scroll and response validation. Empty, duplicate and
excessive sets are rejected; input permutations produce one sorted population.
Restrictive fences must already have removed denied memberships before this
value is built. There is no global or omitted-IDF population variant.

Collection schema identity is internally computed by the #237 canonical CBOR
and BLAKE3 owner. It binds generation, installation incarnation, profile set,
field/index plans, vector mode/requirements, epoch domain, frozen baseline
topology/strictness and fixture/profile versions. Restore recomputes identity
before comparing the expected full digest. A stored digest alone is not an
admitted schema. Payload identity is likewise computed over the closed payload;
it never substitutes for authoritative source/revision readback.

Source donors were inspected at immutable commits:

- #209: `e5c14cde20292fec59e5cabcc921eda3ae5aaa67`, planner model/schema.
- #200: `615d64f70d8b953cc585990659b865cfe250092b`, payload/query/schema/filter.

The existing enum wire strings are retained. Donor branches, local byte writers,
generic crypto owners, physical collection routes and Qdrant SDK types were
not adopted. No new external dependency or lockfile change is needed.

Changed schema/vector/scoring topology requires a fresh collection generation
and a rebuild from retained source/preparation. Old points cannot be adopted,
relabelled or mixed into the new schema. Consumers #259/#260/#262/#263 must
delete their duplicate definitions when they port to this contract. The
indexed spine remains disabled until the separate #264 product qualification.

The PR records exact-head locked check, strict Clippy, nonzero focused fixture
execution and bounded independent source review. Those are contract source
gates; no native, Qdrant, installed or complete product claim follows.
