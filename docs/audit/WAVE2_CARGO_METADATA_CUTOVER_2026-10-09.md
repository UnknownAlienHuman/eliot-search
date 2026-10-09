# Wave 2 Cargo metadata cutover — #250

Base: `889f144b55ff09a9412ed1f461e97b5e084211aa` (accepted #258).
Delivery revision and executed gates are recorded in the delivery PR and #97.

Cargo supplies workspace/default membership, package identity/version/source,
declared dependency contexts and full resolved edges/features. The private
adapter builds official commands with `--locked --offline`, adds
`--all-features` for the graph and uses a separate `--no-deps` status inventory.
It never fetches, updates the lockfile, compiles or executes repository source.
Source/artifact checks do not replace the Cargo graph.

The runner caps stdout at 32 MiB and stderr at 1 MiB, with one 30-second
deadline per invocation. Two readers check growth before append and send one
bounded result each. Exit/UTF-8/parse/size failures are terminal. Owned records
limit packages/nodes to 4096, declared/resolved edges to 32768 and field text
to 16 KiB; formatting conversions are checked before append. This does not
qualify descendant-process cleanup or the Windows Job runner owned by #240.

Production traversal follows Cargo's Normal/Build edges across all targets,
stopping only at the validated workspace bridge ID/path. Direct declarations
are checked for every workspace member and context, including optional, dev,
renamed and target-specific entries. The bridge requires the single exact,
unconditional, nonoptional normal registry dependency. Path/git/other-registry
drift, missing nodes, dangling edges and empty kinds fail closed. Shortest paths
are deterministic; diagnostic truncation is explicit failure.

Remaining TOML reads are narrowly named literal pin/inheritance/unused
patch/replace policy checks and qualification-artifact data. There is no
lockfile parser or recursive manifest-based dependency inference. Existing Rust
source boundary analysis remains until #251. Static test reports identify
`static-policy`; the command identifies `cargo-and-static-policy`. Fallible
JSON serialization has a one-MiB output cap and returns a tool failure.

Schema-v3 status validation checks exact name/path membership, closed fields
and status domains, valid issue references and disposition/enablement
consistency. It scans each workspace-member README with a one-MiB read cap and
detects the banned wording across line breaks. The existing search-eval README
violation is tracked separately by [#324](https://github.com/UnknownAlienHuman/eliot-search/issues/324).
The matrix and product documentation were not rewritten to obtain PASS.

Live owner refresh also found the frozen `ContractBoundsV1::p00` constant
still assigned as a pending migration to closed #258. Its classification and
pending phase remain unchanged; the separate unresolved legacy provenance
review now has an open owner, [#325](https://github.com/UnknownAlienHuman/eliot-search/issues/325).
No legacy bytes or product code changed. The ledger retains all 4654 previous
classifications and adds four exact non-digest text/read slices, for 4658 sites.

Donor archive/source/license/MSRV/feature and bounded advisory evidence is in
[the Wave 2 donor register](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md#250-archive-and-closure-review-2026-10-09).
Compilation and tooling fixtures do not qualify Qdrant, indexed retrieval,
installed standalone operation or product release.
