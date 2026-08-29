# eliot-searchd

**Status:** progressive composition boundary only; runtime behavior is intentionally unimplemented.

`eliot-searchd` is the sole process owner for the Search data root, control journal, provider endpoint
and qualified Qdrant child. Capability logic remains in library packages.

The Cargo feature graph prevents the W1 shell from dragging the complete future system into one agent
context:

- `wave1-shell` — owner, journal and provider framing;
- `wave2-source` — direct source/revision/materialization;
- `wave3-index` — lexical/Qdrant/publication;
- `wave4-query` — bounded query pipeline;
- `wave5-current` — reconciliation/overlay/code;
- `wave6-proof` — exact/subject/comparison;
- `wave7-lifecycle` / `full-baseline` — retention and purge/restore hardening.

The default is `wave1-shell`. A layer is enabled only after accepted dependency handoffs and launch-state
activation.

See [AGENTS.md](AGENTS.md) and
[`swarm/assignments/eliot-searchd.md`](../../swarm/assignments/eliot-searchd.md).
