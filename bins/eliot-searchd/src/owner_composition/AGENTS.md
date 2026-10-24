# Working on daemon owner composition

Read the [architecture](../../../../docs/architecture/README.md),
[implementation guide](../../../../docs/implementation/README.md), local [README](README.md),
[root-owner audit](../../../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md),
[#266](https://github.com/UnknownAlienHuman/eliot-search/issues/266) and
[#360](https://github.com/UnknownAlienHuman/eliot-search/issues/360).

This directory is a consolidation/removal area, not a protected architecture. More than 433 KiB production
Rust currently mixes root locking, initialization, owner slots, command parsing, registry/source/DEK/
migration wrappers and compatibility records.

Implement one explicit sequence:

```text
parsed invocation/settings
→ inspect or explicit manifest-last initialize
→ open current root + one actual process lock
→ open current redb/object/source/Qdrant owners
→ construct handlers
→ serve
→ close owners
→ drop lock
```

Keep actual path/locality/no-reparse/ACL checks, retained root/lock handles and current root-format metadata.
Delete command-shaped `NativeRootRequest`, argv digests/work-stage accounting, dual locks, owner slots/epochs/
succession, lifecycle receipts, initialization journal, direct registry/source/DEK/migration custody and
one-shot/service wrappers with callers.

Invocation parsing belongs to the daemon/CLI. Source, key, control, publication and Qdrant state belongs to
their concrete owners after the guard. Do not create another root manager, operation archive or owner-v2
facade.

Preserve #598's private work and use only its accepted public object/key handoff.

After code, run minimal Rust 1.98 compiler/Clippy for root owner and actual daemon/store consumers, then focused
inspect/initialize/crash-point/second-owner/path-replacement/restart/shutdown cases. Historical owner lifecycle
and compatibility matrices are deleted with their APIs.
