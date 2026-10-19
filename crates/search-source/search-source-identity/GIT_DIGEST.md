# Historical Git stable-identity digest

The old helper derives another SHA-256 value from one admitted repository digest and one exact 20-byte Git
object ID. It is not required by the current source model.

Git object identity already belongs to the repository's selected object format. A current Git source/evidence
binding uses:

```text
registered repository/view
exact commit/tree snapshot
exact tree path
exact blob OID bytes and algorithm
retained blob bytes / ObjectRef
```

A blob OID identifies content, not the logical source path in a tree. Conversely a path/ref/name/remote URL
is not object identity. Keep both exact pieces where the view manifest/source head needs them; do not hash
them into another compatibility identity.

The hard-coded 20-byte input also assumes SHA-1 repositories. The current Git adapter must accept the exact
OID format reported by the selected repository/tooling rather than pad/truncate/relabel it.

Delete `derive_git_stable_identity_digest`, its domain/profile/goldens and this file after the old identity
consumer moves. No migration is required for unsupported pre-release roots.
