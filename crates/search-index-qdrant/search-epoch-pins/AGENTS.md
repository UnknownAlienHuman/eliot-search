# Working on search-epoch-pins

Read the root [AGENTS.md](../../../AGENTS.md), current
[architecture](../../../docs/architecture/README.md), and
[implementation](../../../docs/implementation/README.md). Change the registry,
query/continuation consumers and reclaim admission together when their shared
lifetime behavior changes.

Keep one process-local owner of active guards. Acquire the published view for a
new query; retain or transfer an existing guard for continuation. Hold protection
through the final read that uses that view.

Use the existing query-to-continuation transfer. Remove unused historical
re-acquisition and renewal paths instead of inventing more permits or proofs.
Coordinate final drain with admission; checking an old pin-count snapshot again
does not prevent a later acquisition.

Capacity and TTL bound live or unfinished work. Release completed bindings so a
history of successful cleanup cannot exhaust the registry. A process restart
discards ephemeral guards; durable work resumes from its actual saved inputs.

Verify guard release, continuation transfer, expiry, repeated completed lifecycles
and acquisition racing with retirement through their real callers. Do not keep a
second durable query-lease system or package boundary solely for old fixtures.
