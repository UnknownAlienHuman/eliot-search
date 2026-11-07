# Working on shared values

Read [FUNCTIONS.md](FUNCTIONS.md) and the actual producer/consumer.
Preserve current SourceId/SourceNamespaceId bytes and canonical parsing, ObjectRef digest bytes and the
complete wire-frame bound. Hashing and storage validation belong to the actual object owner.
Do not restore retired plans, receipts, future type catalogues or their exclusive fixtures.
