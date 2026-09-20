# Qdrant adapter boundary

Qdrant is the only vector/index database, but it is a replaceable external
backend. The service must depend on Eliot-owned contracts, not on a particular
`qdrant-client` release.

## Dependency direction

```text
daemon / publication / retrieval / reclaim
                    |
                    v
       search-qdrant-bridge public API
                    |
                    v
        private qdrant-client transport
                    |
                    v
          qualified Qdrant server
```

The boundary is strict:

- only `search-qdrant-bridge` may depend on `qdrant-client`;
- raw `qdrant_client::*` types stay inside that package;
- public bridge signatures use Eliot-owned records and errors;
- collection names, protobuf messages, gRPC status text and vendor filters do
  not cross the package boundary;
- the supervisor owns process lifecycle and supplies a qualified endpoint;
- the daemon composes the bridge but does not construct vendor requests.

Run the structural gate from the repository root:

```powershell
cargo run --locked -p xtask -- validate qdrant-boundary --json
cargo test --locked -p xtask --lib qdrant_boundary::source
cargo test --locked -p xtask --test qdrant_boundary_regressions
cargo test --locked -p xtask --test qdrant_cross_file_boundary
cargo test --locked -p xtask --test qdrant_qualified_path_boundary
cargo test --locked -p xtask --test qdrant_qualification_module_ownership
```

The manifest guard checks both dependency keys and Cargo's `package` rename
field, including target/dev/build/workspace declarations. A renamed SDK
workspace dependency cannot create another allowed consumer. Qdrant `patch`
and `replace` entries are rejected, as are alternate source selectors on the
workspace pin and source/feature overrides on bridge inheritance.

The source guard masks comments and string/character literals before matching
complete reference tokens. Spaces, newlines, intervening comments, grouped
imports and raw identifiers do not hide a direct SDK reference; similarly named
modules such as `qdrant_client_helpers` are not the vendor crate. The file-local
public-surface check follows direct imports and private type aliases and checks
multiline signatures, public trait contracts, enum variants, split
visibility/qualifier layouts and exported macro definitions. Both
`#[macro_export] macro_rules!` and `pub macro` token trees are bounded by their
balanced delimiters; private macros remain adapter internals.

Public-surface extraction uses the same streaming tokens as direct-reference
checking, with original byte offsets and line numbers. Tabs, comments, split
keywords, leading attributes and multiple declarations on one line do not
change signature or field boundaries. Grouped public imports and multiline
named-field types are checked through their balanced delimiters; nested generic
commas, function arrows and const-generic groups cannot prematurely end them.
Function bodies, constant initializers and private named fields are not treated
as public signatures merely because they share a line with `pub`. Nested
exported macro definitions are still inspected. Trait/enum bodies and tuple-
struct signatures (including trailing `where` clauses) remain conservatively
checked as a whole. Traversal is iterative and borrows the masked source rather
than collecting another token vector or copying each surface. The source-test
command above includes the surface regression module; these lexical fixtures do
not compile the synthetic vendor programs or qualify a live backend.

A bounded module-graph pass resolves canonical `lib.rs`, `mod.rs` and nested
`*.rs` layouts, direct literal `include!` edges and direct literal `#[path] mod`
overrides. Included source inherits the including module identity while keeping
its canonical identity for conservative checking. Vendor taint therefore cannot
hide in a split transport file and reappear in an including file's public
signature. Active directives are distinguished from comments and string
lookalikes. Literal targets must resolve inside the retained bridge inventory;
missing targets, direct computed targets, include cycles, excessive semantic
identity growth, excessive module depth and malformed or oversized import trees
fail closed rather than producing a partial PASS.

The cross-file pass resolves `crate`, `self`, repeated `super` and Rust-2018
crate-root paths. It follows renamed/grouped item imports, renamed vendor-crate
roots, chained public re-exports, private imports used by public signatures,
local type-alias chains, public glob re-exports and private glob imports. It
also inspects direct qualified paths in accumulated public signatures,
trait/enum bodies and exported macros, including paths split across lines and
`$crate` macro roots. Public globs propagate only publicly exportable tainted
names; an internal-only vendor binding does not create a false public leak.
Direct vendor-SDK glob imports are rejected because a lexical validator cannot
enumerate their introduced names. Taint keys include full semantic module path
and item name, so an unrelated Eliot-owned type with the same leaf name remains
valid.

The gate also requires the workspace client pin, exactly one `qdrant-client`
record in `Cargo.lock`, `qualified.rs` and `qualification/qdrant/artifact.toml`
to name the same client version, and requires the qualified server version to
match the artifact manifest. Version extraction ignores commented/quoted
lookalike declarations and rejects duplicate active canonical declarations.
The version constants retain their canonical single-line `pub const` string
literal format; unsupported forms fail the comparison instead of being guessed.

The repository walk is deterministic and fail-closed. It rejects observed
symbolic links and unsupported entry types, stops above 64 directory levels or
200,000 entries, and reads at most 16 MiB from one source file and 256 MiB in
total. `.git`, `target`, `.venv`, `__pycache__` and `node_modules` are explicitly
outside the scan. Exceeding a limit is a validation failure, never a partial
`PASS`; the validator does not follow a link or continue after exhausting an
aggregate limit.

This remains a lexical structural check, not complete Rust name resolution,
Cargo compilation or live qualification. Direct computed `include!`/`#[path]`
forms are rejected rather than interpreted. Macro-generated directives,
`cfg_attr`-selected module paths, conditional-compilation truth, module aliases
used as path roots and compiler-derived visibility still require complementary
compiled checks and independent review. A structural PASS alone must not be
represented as complete boundary or product acceptance.

`qualified.rs` is the single upgrade identity facade: server version/build,
artifact digest/size/platform and client version/checksum/VCS identity remain
there together. Bounded `qualified/{artifact,client,error,gate,idf}.rs` modules
verify those identities and admission rules but may not duplicate or own the
pins. This keeps both human updates and the structural validator pointed at one
file while preventing the qualification gate from becoming another monolith.

## Upgrade procedure

A Qdrant upgrade is an adapter qualification change, not a service rewrite.

1. Change the single exact `qdrant-client` pin in the root
   `[workspace.dependencies]`.
2. Update `Cargo.lock` with that exact client release.
3. Update the qualified client/server identities in
   `search-qdrant-bridge/src/qualified.rs` and
   `qualification/qdrant/artifact.toml`.
4. Adapt only private translation code under `search-qdrant-bridge` when the
   vendor protobuf/API changed.
5. Run `xtask validate qdrant-boundary`, the qualification module-ownership
   test, bridge unit/contract tests and the live disposable-server
   qualification.
6. Update the qualification receipt only from executed evidence.

No daemon, query, publication or control-store module may be changed merely to
accommodate a vendor SDK type or method rename. Such a required change means
the adapter boundary leaked and must be repaired before qualification.
