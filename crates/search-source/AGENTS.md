# Source family rules

- Paths are locators, not identity.
- `search-source-admission` is the sole source-admission policy evaluator and legacy v1 receipt owner.
- `search-source-admission-receipt` owns permanent v2 receipt issuance and verification over the evaluator's immutable models; dependencies remain one-way.
- `search-source-registry` stores verified policy bindings/receipts but does not reimplement policy.
- `search-safe-reader` performs stable no-execute acquisition from an already admitted locator; it does
  not decide admission.
- Watchers are hints; inventory reconciliation proves observation continuity.
- Only exact bytes or immutable admitted revisions are source truth.
- `SourceIdentity` carries no membership or access policy.

This directory is not a package. Do not add a family-level `Cargo.toml` or shared implementation. Put
behavior in the child package that owns the failure state and test seam.
