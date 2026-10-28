# eliot-searchd

The intended composition root for one standalone local Search application. Current source still has legacy
interceptors and disconnected subsystems; no default end-to-end product is claimed.

[FUNCTIONS.md](FUNCTIONS.md) maps the entrypoints, owners and removal boundary. [AGENTS.md](AGENTS.md) gives local
editing constraints. Use the [delivery sequence](../../docs/implementation/README.md#delivery-sequence) to select
one connected change instead of enabling every old feature.

The detailed pre-R055 route/feature audit remains in
[Git history](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/bins/eliot-searchd/README.md).
Recheck current callers at the actual code cut. Current execution/build evidence belongs to the
[registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status), not copied status assertions here.
