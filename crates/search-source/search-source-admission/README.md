# search-source-admission

Pure source inclusion policy. It decides whether a selected source may be read and retained; it does not
own the filesystem, source registry, currentness, access control or a receipt platform.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source reality

The v2 policy/observation/evaluator is implemented. Native source acquisition calls
`evaluate_pre_read_v2` before the body read. Full v2 evaluation/receipt reconstruction is routed through
`search-source-provenance` and `search-source-admission-receipt` to preserve original-input/archive
contracts.

The package also retains extensive v1 byte compatibility, canonical receipt/batch identities, a complete
1024-occurrence batch evaluator and many deferred fixtures. Public-main search found no product caller for
`evaluate_batch_v2`. Those mechanisms do not retain one source or make sync work.

## Target policy flow

```text
bounded root-relative locator + metadata
→ pre-read policy
   AllowRead | Deny | ReviewRequired | Unsupported | ObservationIncomplete
→ stable source read only when admitted
→ post-read facts that actually require bytes
→ final retain or explicit gap
```

Known credential/private-key/system and other mandatory safety exclusions remain fail-closed. Post-read
classification may narrow a pre-read allow; it cannot widen a deny or missing observation. The same policy
profile is reused across one inventory operation.

Store only the decision facts needed by current source state and diagnostics: policy/profile revision,
closed outcome/reason and relevant bounded classification. A decision is not a capability for later query
access. The owning local client still searches only its selected published source view.

## No admission lifecycle OS

Ordinary current-format ingestion needs no separately minted receipt, full original-input archive,
canonical decision history or durable batch record. Equal pure inputs can simply be reevaluated. If a
restricted/audited product later requires durable admission evidence, that profile must justify its exact
records without making them prerequisites for local repositories.

Remove v1 compatibility, receipt/provenance leaves and batch identity machinery with their final legacy
consumers. An unsupported old root may be rejected before writes; preserving every pre-release admission
byte is not a baseline requirement.

Keep one reviewed path matcher (`globset`) and one evaluator. Inventory/watcher ignore rules and admission
policy are distinct: the walker avoids irrelevant traversal, while admission enforces safety on every
candidate that reaches the reader.

Verify deny-before-read, policy/locator bounds, incomplete observations, post-read narrowing, cancellation
and content-free errors through the actual ingestion caller. Pure goldens and authored-but-unrun fixtures
do not establish a connected source path.
