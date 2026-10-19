# Historical W7 candidate hardening

The old W7 direction attempted to make caller-created nomination, context, digest, residency, assurance and
phase-permit records authoritative. It is superseded by direct evidence resolution.

Current operation:

```text
ranked stable point
→ pinned PublishedView manifest lookup
→ exact retained object / representation / occurrence / range
→ real immutable-object and range verification
→ VerifiedEvidenceItem or explicit gap
```

Do not add stronger nomination signatures, readback digest capsules, live-security permits, evidence keys or
validation receipts. Echoing expected hashes next to returned bytes never establishes those bytes. The
retained object reader verifies its actual content-address/integrity format; the query owner validates the
manifest range under the same absolute request context.

A point absent, foreign or contradictory in the pinned manifest contaminates the affected backend result.
A present manifest entry whose retained object/range is unavailable becomes a candidate gap. Page selection
occurs before source reads.

Remove the old public validation authority and fixtures with consumers. If manifest lookup plus retained
read remains small, place it in query composition and delete the package. See [FUNCTIONS.md](FUNCTIONS.md).
