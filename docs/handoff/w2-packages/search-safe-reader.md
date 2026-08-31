# W2 package checkpoints — `search-safe-reader`

**Write scope:** `crates/search-source/search-safe-reader/**`  
**Authority:** none; accepted G0/W1, issued ticket, active lease and acknowledged context remain required.

Read only the materialized package context, exact accepted dependency handoffs, assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W2_MILESTONE_PACKETS.md`.

## SR0 — Requests and final-handle containment

Implement configuration, request fences, final-handle/object resolution, admitted-root containment and path/reparse/device denial.

## SR1 — Stable bounded reads

Implement pre/read/post observations, exact byte digest/length, finite retry, oversize rejection, cancellation and no partial success.

## SR2 — Exact local Git-object and batch paths

Implement no-execute exact object reads, missing/remote-object gaps, encoding observation, batch accounting and transient cleanup.

## SR3 — Adversarial and handoff closure

Close symlink/junction/reparse replacement, unstable-file timing, hooks/filters/network canaries, disclosure, line budget and submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, typed gaps, package-only diff,
dependency/API digests and line count. SR3 creates only a package submission candidate.
