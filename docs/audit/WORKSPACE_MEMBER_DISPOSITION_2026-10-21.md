# Workspace disposition

Package count is not an architecture target. Main still contains source removed on unmerged #610; inspect actual
Cargo/module/feature/target and non-Rust callers before deleting anything. [Code map](../implementation/CODE_MAP.md)
locates entrypoints; [working guide](../implementation/WORKING_GUIDE.md) gives the check procedure.

## Keep a boundary only when it isolates

Durable state, an actual external/OS effect, a substantial algorithm or a real optional adapter. Otherwise move
useful code to its consumer and delete the forwarding crate plus exclusive fixtures. No second service is needed.

## Current family direction

| Family | Keep / simplify |
|---|---|
| Root/config/control | One process lock, plain root, small settings and typed redb operations |
| Source/preparation | One acquired-body writer, guarded current rows, exact transformations/ranges |
| Qdrant/publication | One SDK bridge, publisher and view lifetime; remove proof/reclaimer scaffolds after callers move |
| Query | Local source selection, Exact, evidence and bounded windows; no grants/checkpoints/revocation framework |
| Client/CLI | One owned child connection and serializer; remove pairing, endpoint discovery and duplicate sessions |
| Crypto/OS-secrets | Not selected; remove public legacy wiring after real consumers move, without deleting user keys/data |
| Windows listener/transport | Not part of the owned-child baseline; do not implement a second transport first |
| Retention/evaluation | Concrete stopped-root maintenance and later actual product measurements, not generic lifecycle/evidence platforms |
| Optional adapters/workers | Only with an explicit useful consumer; do not block the local lexical path |

## Corrections to the historical map

Earlier R017/R020/R030 removals and the 49-member count describe the refactor branch, not main. The older
310-KiB security-family count is historical inventory, not promised deletion. New instructions select
[plain local scope](../architecture/LOCAL_SCOPE.md), not an optional encrypted handoff. Preserve unpublished work;
recheck actual main consumers, then remove the unused public code with its dependency/fixture closure.
