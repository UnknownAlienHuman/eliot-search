## Result

Issue:

State the one concrete behavior, deletion or documentation correction delivered by this PR. Do not claim a
programme, product or qualification result that was not executed.

## Scope and ownership

```text
primary owner:
real producer:
real consumer:
replaced path:
deleted path:
out of scope:
```

Packages are current locations, not permanent architecture boundaries. A forwarding facade or renamed copy is
not a deletion.

## Compatibility and state

Describe only persisted or externally observable compatibility that actually remains:

```text
current-format behavior:
old-format disposition:
unknown external-effect recovery:
```

Pre-release internal APIs, receipts, generated maps and wave records are not compatibility obligations.
Unsupported old roots must fail before writes and remain untouched.

## Verification

Record raw commands and outcomes. Use the smallest compiler/Clippy closure containing changed production
owners and immediate consumers; run focused behavior/live checks only when that behavior exists.

```text
check:
clippy:
focused runtime behavior:
git diff --check:
not run / unavailable:
```

Do not represent compilation, a constructor, a structural validator or a hard-coded self-test field as
runtime/product evidence.

## Review findings

For each load-bearing finding, record the affected path/symbol, consequence and disposition. Delete obsolete
requirements with their last consumers instead of adding another version or exception registry.

## Evidence boundary

State exactly what this PR proves and what remains unimplemented, unmeasured or owned by another issue.

## Active private work

Does this overlap [#598](https://github.com/UnknownAlienHuman/eliot-search/issues/598)? If not, state `none`.
If it consumes #598, name the accepted public handoff. Do not inspect, reset, rebase, cherry-pick or duplicate
its unpublished branch/worktrees.
