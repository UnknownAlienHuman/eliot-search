# Historical access-policy journal

This file described a pre-release policy mutation stored through the generic
`PersistentControlJournal`. It is **not** a current implementation requirement.

The local owning-account product resolves selected sources from one pinned `PublishedView`; it does not
persist a signed grant or policy operation for every request. A selected restricted-client adapter owns
its real session policy, allowed source set, limits and revocation.

Do not implement or preserve the access-policy receipt, ABA/current-generation proof, snapshot-publisher
checkpoint or dependent security-restriction journal. Remove the old codec/API with its last callers.
Current direction is in [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md) and
[`search-access`](../search-query/search-access/README.md).
