# Qdrant evaluation archive

This directory preserves selected Qdrant 1.19.0 artifact/schema/probe inputs and results from the earlier
W3 qualification programme. They are historical evaluation evidence, not runtime authority and not a gate
that disables an otherwise working current-format product.

- [`W3_QUALIFICATION.md`](W3_QUALIFICATION.md) explains what remains useful and what is retired.
- [`artifact.toml`](artifact.toml) records the selected Windows server/client pair used by those runs.
- [`collection-schema.toml`](collection-schema.toml) records the old frozen probe schema.
- [`probes.toml`](probes.toml) records probe definitions/outcomes.
- `evidence/` and `vendor-boundary/` retain supporting outputs.

Normal startup does not consume these files, manufacture a passing receipt or require `QualifiedGate`.
It connects to the configured service, checks the current server capabilities and verifies the exact
collection generation/schema it will use.

Keep the workspace dependency pin until a deliberate upgrade. Run a focused live suite when changing the
selected Qdrant/client pair or making a concrete behavior claim. Store results as evaluation/release evidence;
do not feed them back into the product constructor or rebuild a permanent acceptance bureaucracy.
