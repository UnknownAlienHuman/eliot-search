# Working on search-handles

Read the [architecture](../../../docs/architecture/README.md),
[query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query) and
[current APIs](FUNCTIONS.md), then trace the actual expansion and lifecycle callers.

Use exact source/revision/range references in the local result. Any required opaque locator shares
the [single temporary window owner](../search-continuation/README.md#one-temporary-owner). Replace both
existing catalogs with that owner; do not make a third registry or preserve a crate solely for its tests.

- Resolve the real retained revision and scope, enforce the requested range and effective byte/time
  limits, and reuse actual-byte verification. Never substitute a digest label for checking bytes.
- Delete expired records and release owned resources; a live-record quota does not bound retained
  terminal history. Keep issued locators usable until their stated expiry.
- Make grant policy specific to the selected restricted profile. Changing trusted-local constructors
  means changing their callers, not filling old fields with synthetic permissions.
- Preserve actual restrictions on excluded sources, purged data and unsaved buffer lifetime.
- Change shared APIs and native/public consumers together. No contract-change permission process,
  durable ordinary-query journal or package line quota is required.
