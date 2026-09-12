# Git stable identity digest

`search-source-identity` owns the pure compatibility formula that binds one
already-admitted repository identity to one exact Git object ID:

```text
SHA-256-domain-frame(
  "eliot-searchd/git-stable-identity/v1",
  repository_identity_digest,
  object_id,
)
```

The crate does not implement SHA-256 itself. The integration boundary supplies
a qualified `GitIdentityDigest`, while this package owns the domain and ordered
framing through `derive_git_stable_identity_digest`.

Load-bearing inputs are only:

- the exact admitted 32-byte repository identity digest;
- the exact validated 20-byte Git object ID.

Repository path, remote URL, repository name, ref, branch, HEAD, worktree path,
logical display path and lineage label are not identity inputs. Lineage evidence
is retained separately and cannot substitute for repository/object identity.

The operation is deterministic, bounded, I/O-free and retry-safe. It performs no
Git command, hook/filter execution, credential-helper invocation, filesystem
read, network fetch, registry mutation, clock access or random ID generation.

Manual check:

```powershell
cargo test --locked -p search-source-identity git_digest
```
