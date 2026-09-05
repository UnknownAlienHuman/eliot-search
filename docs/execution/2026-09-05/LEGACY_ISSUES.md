# T01: legacy issue crosswalk

Observed baseline `a5abdf7ef0cb9d691000759494fd8829b2ba0b60`; fetched open issues on 2026-09-05. All listed issues remain open. Routing is not acceptance or a command to rewrite existing code.

| Existing issues | Continuation | Disposition |
|---|---|---|
| #23, #24 | T01 / #98, T04 / #101 | Do not reimplement contracts from the old scaffold request; #24 is a verifier review, not a product acceptance. |
| #48 | T01 / #98 | UtcTimestamp, MetadataKey and UnresolvedSource need explicit contract/API reconciliation; issue text is a proposal, not an accepted amendment. |
| #54, #63 | T01 / #98, T04 / #101 | One search-domain owner; verify actual API and current evidence, not a second implementation. |
| #55, #64 | T01 / #98, T04 / #101 | One search-ports owner; dependent work consumes accepted public interfaces. |
| #51, #56, #65 | T12 / #109 | Reuse existing pure config mechanics; close effective runtime application and readiness. |
| #53, #59, #79 | T13 / #110 | Actual path crates/search-source/search-source-admission; do not combine side-effecting service and pure admission owners by following issue prose. |
| #60, #81 | T13 / #110 | Actual path crates/search-source/search-source-registry; one canonical registry and cutover owner. |
| #80 | T13 / #110, T32 / #129 | Actual path crates/search-source/search-source-identity; preserve lineage and observed native identity. |
| #61, #73, #89 | T18 / #115, T19 / #116 | No separate authentication/protocol implementations; cryptographic prescription requires accepted version/profile decision. |
| #68, #75, #76, #77, #88 | T04 / #101, T08 / #105, T12 / #109, T19 / #116 | One primary lifecycle and readiness contract; existing experiments are not independent launch instructions. |
| #69, #78 | T19 / #116 | One protocol-only supported CLI; historical development command is not the current product surface. |
| #70, #85 | T09 / #106, T10 / #107, T11 / #108 | PersistentControlJournal exists; finish bounded integration and primary migration, not another redb backend. |
| #71, #86 | T08 / #105 | Actual path crates/search-runtime/search-runtime-owner; unify primary and experimental locking. |
| #72, #87 | T18 / #115 | Actual path crates/search-runtime/search-os-secrets; platform helper ownership needs reconciliation. |
| #74, #90 | T06 / #103, T19 / #116 | Named-pipe-only versus loopback requests are incompatible; settle one accepted transport profile without silent fallback. |
| #82 | T07 / #104, T32 / #129 | Actual path crates/search-source/search-safe-reader; native containment and exact Git acquisition have distinct tasks. |
| #83 | T14 / #111, T37 / #134 | Actual path crates/search-source/search-revision-store; XChaCha prescription cannot silently replace retained DPAPI/AES formats. |
| #84 | T15 / #112 | Actual path crates/search-prep/search-materializer; normalized profile must not overwrite raw-byte coordinate semantics. |
| #93 | T04 / #101 | Run 33721947205 attempt 2 is historical failure; rerunning its old code-generating workflow is not current-head verification. |

The concrete task PRs #98–#140 are the continuation queue. These legacy issues cannot create a second writer lease. Do not close them as completed on the strength of this table. Resolve contract conflicts through the authority map; obtain current-head implementation and review evidence before accepting a task.
