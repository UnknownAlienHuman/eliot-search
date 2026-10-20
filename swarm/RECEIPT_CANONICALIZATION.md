# Retired orchestration receipt format

The signed ticket/lease/handoff/wave receipt canonicalization described by the former swarm control plane is
not a current Search contract. Do not compute, validate or store those records for ordinary implementation,
review or release work.

This retirement does **not** remove product-owned content identities or persisted-format validation. A real
object, source, retrieval profile or protocol owner may still define an exact digest/preimage where its
behavior requires one. Such a digest is tested in that owner and consumed by a real path; it is not entered
into a repository-wide approval ledger or acceptance receipt chain.

Git commit/blob identity plus raw command/runtime artifacts are sufficient for current development evidence.
Historical receipt formats remain in Git and their validators/profiles are removed under #214/#525.
