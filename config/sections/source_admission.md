# Source policy settings disposition

The daemon currently registers only `source_admission.allow_generated` (default `true`); the generic snapshot
does not make it a typed setting of the real ingestion path. The old profile/max-file/vendor/binary catalogue
is not implemented configuration.

Consolidate selected source roots and any real policy fields under one typed source settings owner. Keep
`allow_generated` only if the simplified policy evaluator consumes it. File-size and acquisition limits belong
to the stable reader/materializer path. Known credential/private-key exclusions remain code/policy invariants,
not configurable safety bypasses.

A policy change runs actual reconciliation/publication or requires restart. No policy digest, security-barrier
receipt or implicit membership mutation.
