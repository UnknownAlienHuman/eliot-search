# Working on search-runtime-owner

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [README](README.md) and
[source map](FUNCTIONS.md). Trace the actual daemon root-open and shutdown callers before preserving a
logical owner type.

Keep only behavior needed to initialize or exclusively open one local current-format root:

- explicit initialize versus side-effect-free inspect/open-existing;
- real native/process-lifetime exclusion;
- exact root/install identity needed by the concrete owner;
- honest handling of a real unresolved effect;
- orderly release after concrete owners close.

The daemon owns filesystem/process/control effects. Do not create another owner database, renewable lease,
dependency-receipt graph, generic recovery workflow or supervisor merely to preserve package tests. A
root guard is process-local and need not accompany every internal call.

Change this package and immediate daemon consumers together. Delete obsolete transitions, wrappers and
fixtures with their last caller. No package-only edit wall, historical wave, line quota or compatibility
promise requires keeping an unused lifecycle platform. Unsupported old roots remain untouched.

Check the actual initialize/open/second-owner/shutdown behavior. Pure transition tests do not establish
native exclusion, root safety or a working daemon.
